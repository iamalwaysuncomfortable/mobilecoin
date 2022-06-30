// Copyright (c) 2018-2022 The MobileCoin Foundation

//! A utility for exporting the history of Attestation Verification
//! Reports generated by MobileCoin consensus nodes

use clap::Parser;
use mc_blockchain_types::VerificationReport;
use mc_blockchain_verifiers::{AvrConfig, AvrConfigRecord};
use mc_common::{
    logger::{create_app_logger, o},
    ResponderId,
};
use mc_crypto_keys::Ed25519Public;
use mc_watcher::{error::WatcherDBError, watcher_db::WatcherDB};
use std::{fs, path::PathBuf};
use url::Url;

/// Command line configuration.
#[derive(Parser)]
#[clap(
    name = "mc-watcher-avr-export",
    about = "A utility for exporting the history of MobileCoin consensus enclave AVRs"
)]
pub struct Config {
    /// Path to watcher db (lmdb).
    #[clap(
        long,
        default_value = "/home/ironicflowers/dev/watcher-db",
        parse(from_os_str),
        env = "MC_WATCHER_DB"
    )]
    pub watcher_db: PathBuf,

    /// Path for the avr-history.toml & avr-history.json bootstrap files to be
    /// written.
    #[clap(
        long,
        default_value = "",
        parse(from_os_str),
        env = "MC_AVR_HISTORY_PATH"
    )]
    pub avr_history: PathBuf,
}

fn main() {
    let (logger, _) = create_app_logger(o!());

    let mut config = Config::parse();
    config.avr_history.set_file_name("avr_history");
    let watcher_db =
        WatcherDB::open_ro(&config.watcher_db, logger).expect("Failed opening watcher db");
    let mut avr_records = Vec::new();

    // Get all of the latest synced blocks
    let last_synced_blocks = watcher_db.last_synced_blocks().unwrap();

    // Attempt to reconstruct the AVR history by finding where the AVRs changed
    for (tx_src_url, max_block_index) in last_synced_blocks.iter() {
        let max_block_count = max_block_index.map_or_else(|| 0, |idx| idx + 1);
        let mut cur_start_index = 0;
        let mut cur_end_index = 0;
        let mut cur_signer = None;
        let mut cur_avr: Option<VerificationReport> = None;

        // Check the signer for each block
        for block_index in 0..max_block_count {
            let signer = match watcher_db.get_block_data(tx_src_url, block_index) {
                Ok(block_data) => block_data.signature().map(|sig| *sig.signer()),
                Err(WatcherDBError::NotFound) => None,
                Err(err) => {
                    panic!(
                        "Failed getting block {}@{}: {:?}",
                        cur_start_index, tx_src_url, err
                    );
                }
            };

            if signer == cur_signer {
                cur_end_index += 1;
            } else {
                // If the signer changed, attempt to see if there's a historical AVR.
                // If the found AVR is different (or None), create a new historical
                // record, otherwise, keep searching
                let avr_for_signer = fetch_avr(&watcher_db, tx_src_url, &signer);
                if avr_for_signer.eq(&cur_avr) {
                    cur_end_index += 1
                } else {
                    avr_records.push(AvrConfigRecord::new(
                        &create_responder_id(tx_src_url),
                        cur_start_index,
                        cur_end_index,
                        cur_avr.take(),
                    ));
                    cur_avr = avr_for_signer;
                    cur_start_index = block_index;
                    cur_end_index = block_index;
                }
                cur_signer = signer;
            }
        }
    }

    // If we've found AVR history, write it to disk in both .json and .toml format
    if avr_records.is_empty() {
        println!("No AVR history found to export in WatcherDB");
    } else {
        let avr_reports = AvrConfig::new(avr_records);
        let avr_history_toml = toml::to_string_pretty(&avr_reports).unwrap();
        let avr_history_json = serde_json::to_string_pretty(&avr_reports).unwrap();
        config.avr_history.set_extension("toml");
        fs::write(&config.avr_history, avr_history_toml).unwrap();
        config.avr_history.set_extension("json");
        fs::write(&config.avr_history, avr_history_json).unwrap();
    }
}

// Extract the host name of the consensus node from the archive records
fn create_responder_id(url: &Url) -> ResponderId {
    if url.scheme() == "https" {
        let mut segments = url.path_segments().unwrap();
        let responder = segments.nth_back(1).unwrap().to_string();
        return ResponderId(responder);
    };
    ResponderId(url.to_string())
}

// Attempt to get an AVR from the watcher db for the given signer
fn fetch_avr(
    watcher_db: &WatcherDB,
    tx_src_url: &Url,
    signer: &Option<Ed25519Public>,
) -> Option<VerificationReport> {
    let reports = match signer {
        Some(signer) => watcher_db
            .get_verification_reports_for_signer(signer)
            .expect("Could not get verification reports for signer"),
        None => return None,
    };

    match reports.len() {
        0 => None,
        1 => match reports.get(tx_src_url) {
            Some(reports) => {
                // There should only have one report associated with the signer+url pair
                match reports.len() {
                    0 => None,
                    1 => reports[0].as_ref().cloned(),
                    _ => panic!(
                        "fatal: multiple AVRs found for signing key {:?} from {:?}",
                        signer, tx_src_url
                    ),
                }
            }
            None => None,
        },
        // If there are multiple reports available, DO NOT boostrap from it
        _ => panic!(
            "fatal: multiple AVRs found for signing key {:?} from {:?}",
            signer, tx_src_url
        ),
    }
}

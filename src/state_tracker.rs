use log::{debug, info};
use once_cell::sync::Lazy;
use std::{sync::Mutex, sync::MutexGuard, collections::BTreeMap};
use tokio::time::{sleep, Duration};
use std::time::{SystemTime};
use itertools::Itertools;
use num_bigint::BigUint;

static TIMEOUT: u64 = 60;
static DEAD_TIMEOUT: u64 = 300;

#[derive(Clone, PartialEq)]
pub enum TrStatus {
    Pending,
    Confirmed,
    Expired,
    Aborted
}

#[derive(Clone, PartialEq)]
pub enum TrSubstatus {
    Pending,
    Confirmed,
    Aborted
}

impl std::fmt::Display for TrSubstatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TrSubstatus::Pending => write!(f, "Pending"),
            TrSubstatus::Confirmed => write!(f, "Confirmed"),
            TrSubstatus::Aborted => write!(f, "Aborted")
        }
    }
}

#[derive(Clone)]
pub struct TrSubstate {
    pub service: String,
    pub substatus: TrSubstatus,
}

#[derive(Clone)]
pub struct TransactionState {
    pub id: String,
    pub status: TrStatus,
    pub substates: Vec<TrSubstate>
}

static TRANSACTIONS: Lazy<Mutex<BTreeMap<String, TransactionState>>> =
    Lazy::new(|| { Mutex::new(BTreeMap::new()) });
fn get_transactions()-> MutexGuard<'static, BTreeMap<String, TransactionState>> {
    TRANSACTIONS.lock().unwrap()
}

pub fn read_status(id: &String)-> Option<TrStatus> {
    let transactions = get_transactions();
    let transaction = transactions.get(id)?;
    Some(transaction.status.clone())
}

pub fn update_substatus(id: &String,
                        service: &String,
                        new_substatus: TrSubstatus)
                        -> Result<(), &'static str> {
    let mut transactions = get_transactions();
    let transaction = transactions.get_mut(id)
                                  .ok_or("transaction not found")?;
    let substate = transaction.substates.iter_mut()
                                        .find(|sstate| sstate.service == *service)
                                        .ok_or("unknown service")?;
    substate.substatus = new_substatus;
    actualize_status(transaction);
    Ok(())
}

fn actualize_status(transaction: &mut TransactionState) {
    let subinfo: Vec<String> = transaction.substates
                                          .iter()
                                          .map(|sstate| format!("{}: {}", sstate.service, sstate.substatus) )
                                          .collect();
    info!("transaction {} info => {}", transaction.id, subinfo.iter().join(", "));
    if transaction.substates.iter().any(|sstate| sstate.substatus == TrSubstatus::Aborted) {
        transaction.status = TrStatus::Aborted;
    } else if transaction.substates.iter().all(|sstate| sstate.substatus == TrSubstatus::Confirmed) {
        transaction.status = TrStatus::Confirmed;
    }
}

pub fn create_pending_transaction(services: &Vec<String>)-> String {
    check_expired_transactions();
    let new_id = generate_id(SystemTime::now());
    let substates = services.iter()
                            .map(|service| TrSubstate{ service: service.clone(),
                                                       substatus: TrSubstatus::Pending } )
                            .collect();
    let new_transaction = TransactionState{ id: new_id.clone(),
                                            status: TrStatus::Pending,
                                            substates: substates };
    get_transactions().insert(new_id.clone(), new_transaction);
    new_id
}

fn generate_id(moment: SystemTime)-> String {
    let duration_since_epoch = moment.duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let timestamp_nanos = duration_since_epoch.as_nanos();
    BigUint::from(9999999999999999999 - timestamp_nanos).to_str_radix(36)
}

static TRACKER_PAUSE_INTERVAL: u64 = 2000;

pub async fn run_expiration_tracker() {
    let pause_time = Duration::from_millis(TRACKER_PAUSE_INTERVAL);

    loop {
        debug!("db_state background tracker iterates");
        check_expired_transactions();
        sleep(pause_time).await;
    }
}

// to free a memory
fn check_expired_transactions() {
    let mut transactions = get_transactions();

    let deadlimit_id = generate_id(SystemTime::now() - Duration::from_secs(DEAD_TIMEOUT));
    let deleted = transactions.split_off(&deadlimit_id);

    let mut expired_counter = 0;
    let limit_id = generate_id(SystemTime::now() - Duration::from_secs(TIMEOUT));
    for (&_, transaction) in transactions.range_mut(limit_id..) {
        if transaction.status == TrStatus::Expired {
            break;
        } else if transaction.status == TrStatus::Pending {
            transaction.status = TrStatus::Expired;
            expired_counter += 1;
        }
    }

    debug!("db_state: {}, {}, {}", transactions.len(), deleted.len(), expired_counter);
}

use crate::state_tracker::{TrStatus, TrSubstatus, read_status,
                           update_substatus, create_pending_transaction};
use crate::handler::{TransactionInfo, MessageConfig};

use log::{debug, info};
use tokio::time::{sleep, Duration};
use itertools::Itertools;
use std::{sync::Mutex, sync::MutexGuard, collections::HashMap, fs::File, io::Read};
use once_cell::sync::Lazy;
use yaml_rust::{YamlLoader, Yaml};

pub enum TrErr {
    ArgsError(&'static str),
    Aborted(&'static str),
    Expired(&'static str),
    NotFound(&'static str),
    Unprocessable(&'static str),
    Timeout(&'static str),
    Repeated(&'static str)
}

#[derive(Clone)]
pub struct ServiceConfig {
    pub name: String,
    pub address: String
}

static SERVICE_CONFIGS: Lazy<Mutex<HashMap<String, ServiceConfig>>> =
    Lazy::new(|| { Mutex::new(HashMap::new()) });
fn get_service_configs()-> MutexGuard<'static, HashMap<String, ServiceConfig>> {
    SERVICE_CONFIGS.lock().unwrap()
}

impl std::fmt::Display for ServiceConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Service{{address: {}}}", self.address)
    }
}

pub fn load_service_configs(file: &str)-> Result<(), &'static str> {
    let mut file = File::open(file).or(Err("Unable to open service config file"))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).or(Err("Unable to read service config file"))?;
    let docs = YamlLoader::load_from_str(&contents).or(Err("Error while parsing the yaml file"))?;
    let doc = &docs[0];
    let services = match &doc["services"] {
        Yaml::Array(a) => Ok(a),
        _ => Err("Service list not defined")
    }?;
    let mut service_configs = get_service_configs();
    for service_data in services {
        let new_config = match (&service_data["name"], &service_data["address"]) {
            (Yaml::String(name), Yaml::String(address)) =>
                Ok(ServiceConfig{ name: name.clone(), address: address.clone() }),
            _ =>
                Err("Bad service config")
        }?;
        info!("service loaded: {}", new_config);
        service_configs.insert(new_config.name.clone(), new_config);
    }
    Ok(())
}

pub fn read_service_config(service: &String)-> Option<ServiceConfig> {
    let service_configs = get_service_configs();
    let service_config = service_configs.get(service)?;
    Some(service_config.clone())
}

pub fn create_transaction(tr_data: &TransactionInfo)-> Result<String, String> {
    let services = tr_data.messages.iter()
                                   .flat_map(|msg| msg.services.iter().map(|mc| mc.name.clone()) )
                                   .collect();
    match services.iter().find(|service| read_service_config(service).is_none ) {
        None => (),
        Some(service) => return Err(format!("unknown service: {}", service))
    };

    let id = create_pending_transaction(&services);
    debug!("new transaction {} for {}", id, services.iter().join(", "));
    for msg in &tr_data.messages {
        msg.services.iter().for_each(|msg_config| {
            send_message(msg_config.clone(), id.clone(), msg.data.clone());
        })
    }
    Ok(id)
}

pub async fn confirm_transaction(id: &String, service: &String) -> Result<(), TrErr> {
    match read_status(&id) {
        Some(TrStatus::Pending) => {
            update_substatus(&id, service, TrSubstatus::Confirmed).or_else(|e| Err(TrErr::Unprocessable(e)))?;
            wait_for_confirm(&id).await.or_else(|e| Err(e))?;
            Ok(())
        }
        Some(TrStatus::Aborted) => return Err(TrErr::Aborted("transaction aborted")),
        Some(TrStatus::Expired) => return Err(TrErr::Expired("transaction expired")),
        Some(TrStatus::Confirmed) => return Err(TrErr::Repeated("transaction processed")),
        None => return Err(TrErr::NotFound("transaction not found"))
    }
}

pub fn abort_transaction(id: &String, service: &String) -> Result<(), TrErr> {
    match read_status(&id) {
        Some(TrStatus::Pending) => {
            update_substatus(&id, service, TrSubstatus::Aborted).
                or_else(|e| Err(TrErr::NotFound(e)))?;
            Ok(())
        }
        Some(TrStatus::Aborted) => return Err(TrErr::Repeated("aborted")),
        Some(TrStatus::Expired) => return Err(TrErr::Expired("transaction expired")),
        Some(TrStatus::Confirmed) => return Err(TrErr::ArgsError("transaction processed")),
        None => return Err(TrErr::NotFound("transaction not found"))
    }
}

fn send_message(msg_config: MessageConfig, id: String, data: serde_json::Value) {
    tokio::spawn( async move {
        debug!("sending message for {} to {}", id, &msg_config.name);
        let default_config = match read_service_config(&msg_config.name) { Some(c) => c, _ => return };
        let subpath = msg_config.subpath.or(Some("".to_string())).unwrap();
        let client = reqwest::Client::new();
        let _ = client.post(default_config.address.clone() + &subpath + "/" + &id).json(&data).send().await;
        debug!("message for {} sent to {}/{}", id, &default_config.address, subpath);
    });
}

static PAUSE_INTERVAL: u64 = 64;

pub async fn wait_for_confirm(id: &String)-> Result<(), TrErr> {
    let pause_time = Duration::from_millis(PAUSE_INTERVAL);

    for i in 0..250 {
        debug!("waiting is going on {}", i);
        let _ = match read_status(id) {
            Some(TrStatus::Confirmed) => return Ok(()),
            Some(TrStatus::Pending) => sleep(pause_time).await,
            Some(TrStatus::Expired) => return Err(TrErr::Expired("transaction expired")),
            Some(TrStatus::Aborted) => return Err(TrErr::Aborted("transaction aborted")),
            None => return Err(TrErr::NotFound("transaction not found"))
        };
    }
    Err(TrErr::Timeout("timeout on waiting for status update"))
}

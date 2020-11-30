// Copyright (c) 2019-2020, MASQ (https://masq.ai). All rights reserved.

use rusqlite::{Connection, Error, Statement, Transaction};
use std::fmt::Debug;

// pub trait TransactionWrapper<'a>: Drop {
//     fn commit(&mut self);
// }
//
// pub struct TransactionWrapperReal<'a> {
//     transaction: Transaction<'a>,
// }
//
// impl<'a> TransactionWrapper<'a> for TransactionWrapperReal<'a> {
//     fn commit(&mut self) {
//         unimplemented!()
//     }
// }
//
// impl<'a> Drop for TransactionWrapperReal<'a> {
//     fn drop(&mut self) {
//         unimplemented!()
//     }
// }
//
// impl<'a> From<Transaction<'a>> for TransactionWrapperReal<'a> {
//     fn from(transaction: Transaction<'a>) -> Self {
//         Self { transaction }
//     }
// }

pub trait ConnectionWrapper: Debug + Send {
    fn prepare(&self, query: &str) -> Result<Statement, rusqlite::Error>;
    fn transaction<'a: 'b, 'b>(&'a mut self) -> Result<Transaction<'b>, rusqlite::Error>;
}

#[derive(Debug)]
pub struct ConnectionWrapperReal {
    conn: Connection,
}

impl ConnectionWrapper for ConnectionWrapperReal {
    fn prepare(&self, query: &str) -> Result<Statement, Error> {
        self.conn.prepare(query)
    }
    fn transaction<'a: 'b, 'b>(&'a mut self) -> Result<Transaction<'b>, Error> {
        Ok(self.conn.transaction()?)
    }
}

impl ConnectionWrapperReal {
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }
}
//
// #[cfg(test)]
// mod tests {
//     use masq_lib::test_utils::utils::ensure_node_home_directory_exists;
//     use crate::database::db_initializer::{DbInitializerReal, DbInitializer, CURRENT_SCHEMA_VERSION};
//     use crate::blockchain::blockchain_interface::chain_id_from_name;
//     use crate::db_config::config_dao::{ConfigDaoReal, ConfigDao, ConfigDaoRead};
//
//     #[test]
//     fn commit_works() {
//         let data_dir = ensure_node_home_directory_exists("connection_wrapper", "commit_works");
//         let conn = DbInitializerReal::new().initialize (&data_dir, chain_id_from_name("dev"), true).unwrap();
//         let mut config_dao = ConfigDaoReal::new (conn);
//         {
//             let mut writer = config_dao.start_transaction().unwrap();
//             writer.set("schema_version", Some("booga".to_string())).unwrap();
//             writer.commit().unwrap();
//         }
//
//         let result = config_dao.get ("schema_version").unwrap().value_opt;
//
//         assert_eq! (result, Some ("booga".to_string()));
//     }
//
//     #[test]
//     fn drop_works() {
//         let data_dir = ensure_node_home_directory_exists("connection_wrapper", "commit_works");
//         let conn = DbInitializerReal::new().initialize (&data_dir, chain_id_from_name("dev"), true).unwrap();
//         let mut config_dao = ConfigDaoReal::new (conn);
//         {
//             let mut writer = config_dao.start_transaction().unwrap();
//             writer.set("schema_version", Some("booga".to_string())).unwrap();
//         }
//
//         let result = config_dao.get ("schema_version").unwrap().value_opt;
//
//         assert_eq! (result, Some (CURRENT_SCHEMA_VERSION.to_string()));
//     }
// }
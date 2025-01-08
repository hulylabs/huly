//

use crate::id::{AccId, DeviceId, OrgId, Uid};
use anyhow::Result;
use redb::{Database, MultimapTableDefinition, TableDefinition};
use std::{collections::HashSet, sync::Arc};

#[derive(Debug, Clone)]
pub struct Db {
    db: Arc<Database>,
}

const DEVICE_ACCOUNT: TableDefinition<Uid, Uid> = TableDefinition::new("device_account");
const ACCOUNT_ORGS: MultimapTableDefinition<Uid, Uid> =
    MultimapTableDefinition::new("account_orgs");
const ACCOUNT_DEVICES: MultimapTableDefinition<Uid, Uid> =
    MultimapTableDefinition::new("account_devices");

impl Db {
    pub fn open(path: &str) -> Result<Self> {
        Ok(Self {
            db: Arc::new(Database::open(path)?),
        })
    }

    pub fn create(path: &str) -> Result<Self> {
        Ok(Self {
            db: Arc::new(Database::create(path)?),
        })
    }

    pub fn get_device_account(&self, device: DeviceId) -> Result<Option<AccId>> {
        Ok(self
            .db
            .begin_read()?
            .open_table(DEVICE_ACCOUNT)?
            .get(device)?
            .map(|x| x.value().into()))
    }

    pub fn insert_device_account(&self, device: DeviceId, account: AccId) -> Result<()> {
        let write_tx = self.db.begin_write()?;
        {
            let mut table = write_tx.open_table(DEVICE_ACCOUNT)?;
            table.insert(device, account)?;
        }
        write_tx.commit()?;
        Ok(())
    }

    pub fn get_account_orgs(&self, acc: AccId) -> Result<HashSet<OrgId>> {
        let read_tx = self.db.begin_read()?;
        let table = read_tx.open_multimap_table(ACCOUNT_ORGS)?;

        let mut result = HashSet::<OrgId>::new();
        let mut orgs = table.get(acc)?;

        loop {
            if let Some(org) = orgs.next() {
                result.insert(org?.value().into());
            } else {
                return Ok(result);
            }
        }
    }

    pub fn has_account_device(&self, acc: AccId, device: DeviceId) -> Result<bool> {
        Ok(self
            .db
            .begin_read()?
            .open_multimap_table(ACCOUNT_DEVICES)?
            .get(acc)?
            .any(|v| v.map(|x| device.as_bytes() == &x.value()).unwrap_or(false)))
    }
}

//     fn put_account(&self, account: &AccountId, message: &Message) -> Result<()> {
//         let write_tx = self.db.begin_write()?;
//         {
//             let mut table = write_tx.open_table(ACCOUNT)?;
//             table.insert(account, message.to_raw())?;
//         }
//         write_tx.commit()?;
//         Ok(())
//     }

//     fn get_document(&self, document: &DocId) -> anyhow::Result<Option<Message>> {
//         let read_tx = self.db.begin_read()?;
//         let table = read_tx.open_table(DOCUMENT)?;
//         table
//             .get(document)?
//             .map(|x| Message::try_from(x.value()))
//             .transpose()
//     }

//     fn put_document(&self, document: &DocId, message: Message) -> Result<()> {
//         let write_tx = self.db.begin_write()?;
//         {
//             let mut table = write_tx.open_table(DOCUMENT)?;
//             table.insert(document, message.to_raw())?;
//         }
//         write_tx.commit()?;
//         Ok(())
//     }

//     fn get_followers(&self, doc: &DocId) -> Result<HashSet<AccountId>> {
//         let read_tx = self.db.begin_read()?;
//         let table = read_tx.open_multimap_table(FOLLOWER)?;

//         let mut result: HashSet<AccountId> = HashSet::new();
//         let mut followers = table.get(doc)?;

//         loop {
//             if let Some(id) = followers.next() {
//                 result.insert(AccountId::from_bytes(id?.value()));
//             } else {
//                 return Ok(result);
//             }
//         }
//     }

//     fn add_follower(&self, doc: &DocId, account: &AccountId) -> Result<()> {
//         let write_tx = self.db.begin_write()?;
//         {
//             let mut table = write_tx.open_multimap_table(FOLLOWER)?;
//             table.insert(doc, account)?;
//         }
//         write_tx.commit()?;
//         Ok(())
//     }

//     // fn get_object(&self, uuid: &Uuid) -> anyhow::Result<Option<Message>> {
//     //     let read_tx = self.db.begin_read()?;
//     //     let table = read_tx.open_table(OBJECTS)?;
//     //     table
//     //         .get(uuid.as_bytes())?
//     //         .map(|x| Message::try_from(x.value()))
//     //         .transpose()
//     // }

//     // fn put_object(&self, uuid: &Uuid, message: &Message) -> Result<()> {
//     //     let write_tx = self.db.begin_write()?;
//     //     {
//     //         let mut table = write_tx.open_table(OBJECTS)?;
//     //         table.insert(uuid.as_bytes(), message.as_raw())?;
//     //     }
//     //     write_tx.commit()?;
//     //     Ok(())
//     // }

//     // fn get_activity(&self, uuid: &Uuid) -> Result<Box<dyn Iterator<Item = Result<Message>>>> {
//     //     let read_tx = self.db.begin_read()?;
//     //     let table = read_tx.open_table(ACTIVITY)?;
//     //     let uuid = uuid.as_bytes();
//     //     let iter = table
//     //         .range((uuid, 0)..(uuid, 100))?
//     //         .map(|access| match access {
//     //             Ok((_, val)) => Message::try_from(val.value()),
//     //             Err(e) => anyhow::bail!(e),
//     //         });
//     //     Ok(Box::new(iter))
//     // }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::id::{AccountId, DocId};
//     use crate::model::{Format, Message};
//     use tempfile::TempDir;

//     fn setup_store() -> (TempDir, RedbStore) {
//         let tmp_dir = TempDir::new().expect("Failed to create temp dir");
//         let db_path = tmp_dir.path().join("test.db");
//         create_db(db_path.to_str().unwrap()).expect("Failed to create database");
//         let store = RedbStore::open(db_path.to_str().unwrap()).expect("Failed to open database");

//         // Create tables
//         let write_tx = store
//             .db
//             .begin_write()
//             .expect("Failed to begin write transaction");
//         {
//             write_tx
//                 .open_table(ACCOUNT)
//                 .expect("Failed to create account table");
//             write_tx
//                 .open_table(DOCUMENT)
//                 .expect("Failed to create document table");
//             write_tx
//                 .open_multimap_table(FOLLOWER)
//                 .expect("Failed to create follower table");
//         }
//         write_tx.commit().expect("Failed to commit transaction");

//         (tmp_dir, store)
//     }

//     #[test]
//     fn test_redb_store_put_get_account() {
//         // Arrange
//         let (_tmp_dir, store) = setup_store();
//         let account_id = AccountId::new();
//         let msg = Message::new(Format::Json, br#"{"test":"data"}"#.to_vec().into());

//         // Act
//         store
//             .put_account(&account_id, &msg)
//             .expect("Failed to put account");
//         let retrieved = store
//             .get_account(&account_id)
//             .expect("Failed to get account");

//         // Assert
//         assert!(retrieved.is_some());
//         let retrieved_msg = retrieved.unwrap();
//         assert_eq!(retrieved_msg.format(), Format::Json);
//         assert_eq!(retrieved_msg.bytes(), br#"{"test":"data"}"#);
//     }

//     #[test]
//     fn test_redb_store_get_nonexistent_account() {
//         // Arrange
//         let (_tmp_dir, store) = setup_store();
//         let account_id = AccountId::new();

//         // Act
//         let result = store
//             .get_account(&account_id)
//             .expect("Failed to attempt get_account");

//         // Assert
//         assert!(result.is_none());
//     }

//     #[test]
//     fn test_redb_store_put_get_document() {
//         // Arrange
//         let (_tmp_dir, store) = setup_store();
//         let doc_id = DocId::new();
//         let msg = Message::new(
//             Format::Json,
//             br#"{"content":"test document"}"#.to_vec().into(),
//         );

//         // Act
//         store
//             .put_document(&doc_id, msg.clone())
//             .expect("Failed to put document");
//         let retrieved = store.get_document(&doc_id).expect("Failed to get document");

//         // Assert
//         assert!(retrieved.is_some());
//         let retrieved_msg = retrieved.unwrap();
//         assert_eq!(retrieved_msg.format(), Format::Json);
//         assert_eq!(retrieved_msg.bytes(), br#"{"content":"test document"}"#);
//     }

//     #[test]
//     fn test_redb_store_followers() {
//         // Arrange
//         let (_tmp_dir, store) = setup_store();
//         let doc_id = DocId::new();
//         let account_ids: Vec<AccountId> = (0..3).map(|_| AccountId::new()).collect();

//         // Act - Add followers
//         for account_id in &account_ids {
//             store
//                 .add_follower(&doc_id, account_id)
//                 .expect("Failed to add follower");
//         }

//         // Get followers
//         let followers = store
//             .get_followers(&doc_id)
//             .expect("Failed to get followers");

//         // Assert
//         assert_eq!(followers.len(), 3);
//         for account_id in account_ids {
//             assert!(followers.contains(&account_id));
//         }
//     }

//     #[test]
//     fn test_redb_store_empty_followers() {
//         // Arrange
//         let (_tmp_dir, store) = setup_store();
//         let doc_id = DocId::new();

//         // Act
//         let followers = store
//             .get_followers(&doc_id)
//             .expect("Failed to get followers");

//         // Assert
//         assert!(followers.is_empty());
//     }

//     #[test]
//     fn test_redb_store_multiple_formats() {
//         // Arrange
//         let (_tmp_dir, store) = setup_store();
//         let account_id = AccountId::new();

//         // Test different formats
//         let formats = vec![
//             (Format::Json, br#"{"test":"json"}"#.to_vec()),
//             (Format::Yaml, b"test: yaml".to_vec()),
//             (Format::CapnpBinary, b"binary data".to_vec()),
//             (Format::CapnpPacked, b"packed data".to_vec()),
//         ];

//         for (format, data) in formats {
//             // Act
//             let msg = Message::new(format, data.clone().into());
//             store
//                 .put_account(&account_id, &msg)
//                 .expect("Failed to put account");
//             let retrieved = store
//                 .get_account(&account_id)
//                 .expect("Failed to get account");

//             // Assert
//             assert!(retrieved.is_some());
//             let retrieved_msg = retrieved.unwrap();
//             assert_eq!(retrieved_msg.format(), format);
//             assert_eq!(retrieved_msg.bytes(), data);
//         }
//     }

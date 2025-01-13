crates/huly/src/client.rs
```
1 | //
2 | 
3 | use crate::id::{AccId, OrgId};
4 | use crate::membership::{Membership, MembershipRequest, ServeMeRequest};
5 | use crate::message::{Message, MessageType, SignedMessage};
6 | use anyhow::Result;
7 | use futures_lite::StreamExt;
8 | use iroh::{Endpoint, NodeId, SecretKey};
9 | use iroh_gossip::{net::Gossip, proto::TopicId};
10 | 
11 | pub async fn request_membership(
12 |     secret_key: &SecretKey,
13 |     endpoint: Endpoint,
14 |     account: AccId,
15 |     org: OrgId,
16 |     gossip: Gossip,
17 | ) -> Result<()> {
18 |     let node_id = NodeId::from_bytes(org.as_bytes())?;
19 |     let conn = endpoint.connect(node_id, Membership::ALPN).await?;
20 |     let (mut send, mut recv) = conn.open_bi().await?;
21 | 
22 |     let request = MembershipRequest::new(secret_key.public().into(), account, org);
23 |     let encoded = MembershipRequest::encode(&request)?;
24 |     let signed = SignedMessage::sign(secret_key, encoded)?;
25 |     let encoded = SignedMessage::encode(&signed)?;
26 | 
27 |     encoded.write_async(&mut send).await?;
28 |     println!("sent membership request: {:?}", request);
29 | 
30 |     let response = Message::read_async(&mut recv).await?;
31 |     println!("got membership response: {:?}", response);
32 | 
33 |     let topic = TopicId::from_bytes(account.into());
34 |     // let gossip = Gossip::builder().spawn(endpoint.clone()).await?;
35 | 
36 |     let (_sender, mut receiver) = gossip.subscribe(topic, vec![node_id])?.split();
37 | 
38 |     println!("started gossip proto");
39 | 
40 |     // Spawn a task to handle received messages
41 |     let gossip_handle = tokio::spawn(async move {
42 |         while let Some(event) = receiver.try_next().await? {
43 |             println!("Client received gossip event: {:?}", event);
44 |         }
45 |         Ok::<_, anyhow::Error>(())
46 |     });
47 | 
48 |     let request = ServeMeRequest::encode(&ServeMeRequest {})?;
49 |     request.write_async(&mut send).await?;
50 |     println!("sent serve me request");
51 | 
52 |     let response = Message::read_async(&mut recv).await?;
53 |     println!("got serve me response: {:?}", response);
54 | 
55 |     // Wait for Ctrl-C while keeping gossip connection alive
56 |     tokio::select! {
57 |         _ = tokio::signal::ctrl_c() => {
58 |             println!("Received Ctrl-C, shutting down");
59 |         }
60 |         res = gossip_handle => {
61 |             if let Err(e) = res {
62 |                 println!("Gossip task error: {:?}", e);
63 |             }
64 |         }
65 |     }
66 | 
67 |     send.finish()?;
68 |     send.stopped().await?;
69 | 
70 |     Ok(())
71 | }
```

crates/huly/src/config.rs
```
1 | // Huly™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // config.rs
4 | 
5 | use iroh::{PublicKey, SecretKey};
6 | use once_cell::sync::OnceCell;
7 | 
8 | static NODE_CONFIG: OnceCell<Config> = OnceCell::new();
9 | 
10 | #[derive(Debug)]
11 | pub struct Config {
12 |     secret_key: SecretKey,
13 | }
14 | 
15 | impl Config {
16 |     pub fn new(secret_key: [u8; 32]) -> Self {
17 |         Self {
18 |             secret_key: SecretKey::from(secret_key),
19 |         }
20 |     }
21 | 
22 |     pub fn public(&self) -> PublicKey {
23 |         self.secret_key.public()
24 |     }
25 | }
26 | 
27 | pub fn initialize(secret_key: [u8; 32]) {
28 |     NODE_CONFIG.set(Config::new(secret_key)).unwrap();
29 | }
30 | 
31 | pub fn get_state() -> &'static Config {
32 |     NODE_CONFIG.get().expect("node not configured")
33 | }
```

crates/huly/src/db.rs
```
1 | //
2 | 
3 | use crate::id::{AccId, DeviceId, OrgId, Uid};
4 | use anyhow::Result;
5 | use redb::{Database, MultimapTableDefinition, TableDefinition};
6 | use std::{collections::HashSet, sync::Arc};
7 | 
8 | #[derive(Debug, Clone)]
9 | pub struct Db {
10 |     db: Arc<Database>,
11 | }
12 | 
13 | const DEVICE_ACCOUNT: TableDefinition<Uid, Uid> = TableDefinition::new("device_account");
14 | const ACCOUNT_ORGS: MultimapTableDefinition<Uid, Uid> =
15 |     MultimapTableDefinition::new("account_orgs");
16 | const ACCOUNT_DEVICES: MultimapTableDefinition<Uid, Uid> =
17 |     MultimapTableDefinition::new("account_devices");
18 | 
19 | impl Db {
20 |     pub fn open(path: &str) -> Result<Self> {
21 |         Ok(Self {
22 |             db: Arc::new(Database::open(path)?),
23 |         })
24 |     }
25 | 
26 |     pub fn create(path: &str) -> Result<Self> {
27 |         Ok(Self {
28 |             db: Arc::new(Database::create(path)?),
29 |         })
30 |     }
31 | 
32 |     pub fn get_device_account(&self, device: DeviceId) -> Result<Option<AccId>> {
33 |         Ok(self
34 |             .db
35 |             .begin_read()?
36 |             .open_table(DEVICE_ACCOUNT)?
37 |             .get(device)?
38 |             .map(|x| x.value().into()))
39 |     }
40 | 
41 |     pub fn insert_device_account(&self, device: DeviceId, account: AccId) -> Result<()> {
42 |         let write_tx = self.db.begin_write()?;
43 |         {
44 |             let mut table = write_tx.open_table(DEVICE_ACCOUNT)?;
45 |             table.insert(device, account)?;
46 |         }
47 |         write_tx.commit()?;
48 |         Ok(())
49 |     }
50 | 
51 |     pub fn get_account_orgs(&self, acc: AccId) -> Result<HashSet<OrgId>> {
52 |         let read_tx = self.db.begin_read()?;
53 |         let table = read_tx.open_multimap_table(ACCOUNT_ORGS)?;
54 | 
55 |         let mut result = HashSet::<OrgId>::new();
56 |         let mut orgs = table.get(acc)?;
57 | 
58 |         loop {
59 |             if let Some(org) = orgs.next() {
60 |                 result.insert(org?.value().into());
61 |             } else {
62 |                 return Ok(result);
63 |             }
64 |         }
65 |     }
66 | 
67 |     pub fn has_account_device(&self, acc: AccId, device: DeviceId) -> Result<bool> {
68 |         Ok(self
69 |             .db
70 |             .begin_read()?
71 |             .open_multimap_table(ACCOUNT_DEVICES)?
72 |             .get(acc)?
73 |             .any(|v| v.map(|x| device.as_bytes() == &x.value()).unwrap_or(false)))
74 |     }
75 | }
76 | 
77 | //     fn put_account(&self, account: &AccountId, message: &Message) -> Result<()> {
78 | //         let write_tx = self.db.begin_write()?;
79 | //         {
80 | //             let mut table = write_tx.open_table(ACCOUNT)?;
81 | //             table.insert(account, message.to_raw())?;
82 | //         }
83 | //         write_tx.commit()?;
84 | //         Ok(())
85 | //     }
86 | 
87 | //     fn get_document(&self, document: &DocId) -> anyhow::Result<Option<Message>> {
88 | //         let read_tx = self.db.begin_read()?;
89 | //         let table = read_tx.open_table(DOCUMENT)?;
90 | //         table
91 | //             .get(document)?
92 | //             .map(|x| Message::try_from(x.value()))
93 | //             .transpose()
94 | //     }
95 | 
96 | //     fn put_document(&self, document: &DocId, message: Message) -> Result<()> {
97 | //         let write_tx = self.db.begin_write()?;
98 | //         {
99 | //             let mut table = write_tx.open_table(DOCUMENT)?;
100 | //             table.insert(document, message.to_raw())?;
101 | //         }
102 | //         write_tx.commit()?;
103 | //         Ok(())
104 | //     }
105 | 
106 | //     fn get_followers(&self, doc: &DocId) -> Result<HashSet<AccountId>> {
107 | //         let read_tx = self.db.begin_read()?;
108 | //         let table = read_tx.open_multimap_table(FOLLOWER)?;
109 | 
110 | //         let mut result: HashSet<AccountId> = HashSet::new();
111 | //         let mut followers = table.get(doc)?;
112 | 
113 | //         loop {
114 | //             if let Some(id) = followers.next() {
115 | //                 result.insert(AccountId::from_bytes(id?.value()));
116 | //             } else {
117 | //                 return Ok(result);
118 | //             }
119 | //         }
120 | //     }
121 | 
122 | //     fn add_follower(&self, doc: &DocId, account: &AccountId) -> Result<()> {
123 | //         let write_tx = self.db.begin_write()?;
124 | //         {
125 | //             let mut table = write_tx.open_multimap_table(FOLLOWER)?;
126 | //             table.insert(doc, account)?;
127 | //         }
128 | //         write_tx.commit()?;
129 | //         Ok(())
130 | //     }
131 | 
132 | //     // fn get_object(&self, uuid: &Uuid) -> anyhow::Result<Option<Message>> {
133 | //     //     let read_tx = self.db.begin_read()?;
134 | //     //     let table = read_tx.open_table(OBJECTS)?;
135 | //     //     table
136 | //     //         .get(uuid.as_bytes())?
137 | //     //         .map(|x| Message::try_from(x.value()))
138 | //     //         .transpose()
139 | //     // }
140 | 
141 | //     // fn put_object(&self, uuid: &Uuid, message: &Message) -> Result<()> {
142 | //     //     let write_tx = self.db.begin_write()?;
143 | //     //     {
144 | //     //         let mut table = write_tx.open_table(OBJECTS)?;
145 | //     //         table.insert(uuid.as_bytes(), message.as_raw())?;
146 | //     //     }
147 | //     //     write_tx.commit()?;
148 | //     //     Ok(())
149 | //     // }
150 | 
151 | //     // fn get_activity(&self, uuid: &Uuid) -> Result<Box<dyn Iterator<Item = Result<Message>>>> {
152 | //     //     let read_tx = self.db.begin_read()?;
153 | //     //     let table = read_tx.open_table(ACTIVITY)?;
154 | //     //     let uuid = uuid.as_bytes();
155 | //     //     let iter = table
156 | //     //         .range((uuid, 0)..(uuid, 100))?
157 | //     //         .map(|access| match access {
158 | //     //             Ok((_, val)) => Message::try_from(val.value()),
159 | //     //             Err(e) => anyhow::bail!(e),
160 | //     //         });
161 | //     //     Ok(Box::new(iter))
162 | //     // }
163 | // }
164 | 
165 | // #[cfg(test)]
166 | // mod tests {
167 | //     use super::*;
168 | //     use crate::id::{AccountId, DocId};
169 | //     use crate::model::{Format, Message};
170 | //     use tempfile::TempDir;
171 | 
172 | //     fn setup_store() -> (TempDir, RedbStore) {
173 | //         let tmp_dir = TempDir::new().expect("Failed to create temp dir");
174 | //         let db_path = tmp_dir.path().join("test.db");
175 | //         create_db(db_path.to_str().unwrap()).expect("Failed to create database");
176 | //         let store = RedbStore::open(db_path.to_str().unwrap()).expect("Failed to open database");
177 | 
178 | //         // Create tables
179 | //         let write_tx = store
180 | //             .db
181 | //             .begin_write()
182 | //             .expect("Failed to begin write transaction");
183 | //         {
184 | //             write_tx
185 | //                 .open_table(ACCOUNT)
186 | //                 .expect("Failed to create account table");
187 | //             write_tx
188 | //                 .open_table(DOCUMENT)
189 | //                 .expect("Failed to create document table");
190 | //             write_tx
191 | //                 .open_multimap_table(FOLLOWER)
192 | //                 .expect("Failed to create follower table");
193 | //         }
194 | //         write_tx.commit().expect("Failed to commit transaction");
195 | 
196 | //         (tmp_dir, store)
197 | //     }
198 | 
199 | //     #[test]
200 | //     fn test_redb_store_put_get_account() {
201 | //         // Arrange
202 | //         let (_tmp_dir, store) = setup_store();
203 | //         let account_id = AccountId::new();
204 | //         let msg = Message::new(Format::Json, br#"{"test":"data"}"#.to_vec().into());
205 | 
206 | //         // Act
207 | //         store
208 | //             .put_account(&account_id, &msg)
209 | //             .expect("Failed to put account");
210 | //         let retrieved = store
211 | //             .get_account(&account_id)
212 | //             .expect("Failed to get account");
213 | 
214 | //         // Assert
215 | //         assert!(retrieved.is_some());
216 | //         let retrieved_msg = retrieved.unwrap();
217 | //         assert_eq!(retrieved_msg.format(), Format::Json);
218 | //         assert_eq!(retrieved_msg.bytes(), br#"{"test":"data"}"#);
219 | //     }
220 | 
221 | //     #[test]
222 | //     fn test_redb_store_get_nonexistent_account() {
223 | //         // Arrange
224 | //         let (_tmp_dir, store) = setup_store();
225 | //         let account_id = AccountId::new();
226 | 
227 | //         // Act
228 | //         let result = store
229 | //             .get_account(&account_id)
230 | //             .expect("Failed to attempt get_account");
231 | 
232 | //         // Assert
233 | //         assert!(result.is_none());
234 | //     }
235 | 
236 | //     #[test]
237 | //     fn test_redb_store_put_get_document() {
238 | //         // Arrange
239 | //         let (_tmp_dir, store) = setup_store();
240 | //         let doc_id = DocId::new();
241 | //         let msg = Message::new(
242 | //             Format::Json,
243 | //             br#"{"content":"test document"}"#.to_vec().into(),
244 | //         );
245 | 
246 | //         // Act
247 | //         store
248 | //             .put_document(&doc_id, msg.clone())
249 | //             .expect("Failed to put document");
250 | //         let retrieved = store.get_document(&doc_id).expect("Failed to get document");
251 | 
252 | //         // Assert
253 | //         assert!(retrieved.is_some());
254 | //         let retrieved_msg = retrieved.unwrap();
255 | //         assert_eq!(retrieved_msg.format(), Format::Json);
256 | //         assert_eq!(retrieved_msg.bytes(), br#"{"content":"test document"}"#);
257 | //     }
258 | 
259 | //     #[test]
260 | //     fn test_redb_store_followers() {
261 | //         // Arrange
262 | //         let (_tmp_dir, store) = setup_store();
263 | //         let doc_id = DocId::new();
264 | //         let account_ids: Vec<AccountId> = (0..3).map(|_| AccountId::new()).collect();
265 | 
266 | //         // Act - Add followers
267 | //         for account_id in &account_ids {
268 | //             store
269 | //                 .add_follower(&doc_id, account_id)
270 | //                 .expect("Failed to add follower");
271 | //         }
272 | 
273 | //         // Get followers
274 | //         let followers = store
275 | //             .get_followers(&doc_id)
276 | //             .expect("Failed to get followers");
277 | 
278 | //         // Assert
279 | //         assert_eq!(followers.len(), 3);
280 | //         for account_id in account_ids {
281 | //             assert!(followers.contains(&account_id));
282 | //         }
283 | //     }
284 | 
285 | //     #[test]
286 | //     fn test_redb_store_empty_followers() {
287 | //         // Arrange
288 | //         let (_tmp_dir, store) = setup_store();
289 | //         let doc_id = DocId::new();
290 | 
291 | //         // Act
292 | //         let followers = store
293 | //             .get_followers(&doc_id)
294 | //             .expect("Failed to get followers");
295 | 
296 | //         // Assert
297 | //         assert!(followers.is_empty());
298 | //     }
299 | 
300 | //     #[test]
301 | //     fn test_redb_store_multiple_formats() {
302 | //         // Arrange
303 | //         let (_tmp_dir, store) = setup_store();
304 | //         let account_id = AccountId::new();
305 | 
306 | //         // Test different formats
307 | //         let formats = vec![
308 | //             (Format::Json, br#"{"test":"json"}"#.to_vec()),
309 | //             (Format::Yaml, b"test: yaml".to_vec()),
310 | //             (Format::CapnpBinary, b"binary data".to_vec()),
311 | //             (Format::CapnpPacked, b"packed data".to_vec()),
312 | //         ];
313 | 
314 | //         for (format, data) in formats {
315 | //             // Act
316 | //             let msg = Message::new(format, data.clone().into());
317 | //             store
318 | //                 .put_account(&account_id, &msg)
319 | //                 .expect("Failed to put account");
320 | //             let retrieved = store
321 | //                 .get_account(&account_id)
322 | //                 .expect("Failed to get account");
323 | 
324 | //             // Assert
325 | //             assert!(retrieved.is_some());
326 | //             let retrieved_msg = retrieved.unwrap();
327 | //             assert_eq!(retrieved_msg.format(), format);
328 | //             assert_eq!(retrieved_msg.bytes(), data);
329 | //         }
330 | //     }
```

crates/huly/src/id.rs
```
1 | // Huly™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | 
3 | use anyhow::Result;
4 | use iroh::PublicKey;
5 | use serde::{Deserialize, Serialize};
6 | use std::borrow::Borrow;
7 | use std::fmt::{Debug, Display};
8 | use std::str::FromStr;
9 | 
10 | const LENGTH: usize = 32;
11 | 
12 | // we have two types of identities: Hash and PublicKey
13 | // both are represented as 32-byte arrays
14 | pub type Uid = [u8; 32];
15 | pub type Hash = [u8; 32];
16 | 
17 | #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
18 | pub struct PKey(Uid);
19 | 
20 | impl PKey {
21 |     pub fn as_bytes(&self) -> &[u8; 32] {
22 |         &self.0
23 |     }
24 | }
25 | 
26 | impl From<PublicKey> for PKey {
27 |     fn from(key: PublicKey) -> Self {
28 |         Self(*key.as_bytes())
29 |     }
30 | }
31 | 
32 | impl From<PKey> for PublicKey {
33 |     fn from(val: PKey) -> Self {
34 |         PublicKey::from_bytes(&val.0).expect("no way")
35 |     }
36 | }
37 | 
38 | impl From<Uid> for PKey {
39 |     fn from(uid: Uid) -> Self {
40 |         Self(uid)
41 |     }
42 | }
43 | 
44 | impl Borrow<Uid> for PKey {
45 |     fn borrow(&self) -> &[u8; 32] {
46 |         &self.0
47 |     }
48 | }
49 | 
50 | impl From<PKey> for Uid {
51 |     fn from(key: PKey) -> Self {
52 |         key.0
53 |     }
54 | }
55 | 
56 | impl Display for PKey {
57 |     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
58 |         write!(f, "PublicKey({})", data_encoding::HEXLOWER.encode(&self.0))
59 |     }
60 | }
61 | 
62 | fn decode_base32_hex(s: &str) -> Result<[u8; 32]> {
63 |     let mut bytes = [0u8; 32];
64 | 
65 |     let res = if s.len() == LENGTH * 2 {
66 |         data_encoding::HEXLOWER.decode_mut(s.as_bytes(), &mut bytes)
67 |     } else {
68 |         data_encoding::BASE32_NOPAD.decode_mut(s.to_ascii_uppercase().as_bytes(), &mut bytes)
69 |     };
70 |     match res {
71 |         Ok(len) => {
72 |             if len != LENGTH {
73 |                 anyhow::bail!("invalid length");
74 |             }
75 |         }
76 |         Err(partial) => return Err(partial.error.into()),
77 |     }
78 |     Ok(bytes)
79 | }
80 | 
81 | impl FromStr for PKey {
82 |     type Err = anyhow::Error;
83 | 
84 |     fn from_str(s: &str) -> Result<Self, Self::Err> {
85 |         Ok(PKey(decode_base32_hex(s)?))
86 |     }
87 | }
88 | 
89 | pub type AccId = PKey;
90 | pub type OrgId = PKey;
91 | 
92 | //
93 | 
94 | pub type ObjId = PKey;
95 | pub type NodeId = PKey;
96 | pub type DeviceId = NodeId;
```

crates/huly/src/lib.rs
```
1 | // Huly™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | 
3 | pub mod client;
4 | pub mod config;
5 | pub mod db;
6 | pub mod id;
7 | pub mod membership;
8 | pub mod message;
```

crates/huly/src/membership.rs
```
1 | //
2 | 
3 | use crate::db::Db;
4 | use crate::id::{AccId, NodeId, OrgId};
5 | use crate::message::{Message, MessageType, SignedMessage, Timestamp};
6 | use anyhow::Result;
7 | use futures_lite::future::Boxed as BoxedFuture;
8 | use futures_lite::StreamExt;
9 | use iroh::endpoint::{get_remote_node_id, Connecting};
10 | use iroh::protocol::ProtocolHandler;
11 | use iroh::Endpoint;
12 | use iroh_gossip::net::GossipSender;
13 | use iroh_gossip::{
14 |     net::{Event, Gossip, GossipEvent, GossipReceiver},
15 |     proto::TopicId,
16 | };
17 | use serde::{Deserialize, Serialize};
18 | use std::sync::Arc;
19 | 
20 | #[derive(Debug, Clone)]
21 | pub struct Membership {
22 |     db: Db,
23 |     endpoint: Endpoint,
24 |     gossip: Gossip,
25 | }
26 | 
27 | impl Membership {
28 |     pub const ALPN: &[u8] = b"huly/membership/0";
29 | 
30 |     pub fn new(db: Db, endpoint: Endpoint, gossip: Gossip) -> Arc<Self> {
31 |         Arc::new(Self {
32 |             db,
33 |             endpoint,
34 |             gossip,
35 |         })
36 |     }
37 | }
38 | 
39 | async fn account_loop(_sender: GossipSender, mut receiver: GossipReceiver) -> Result<()> {
40 |     println!("Account loop started");
41 |     while let Some(event) = receiver.try_next().await? {
42 |         println!("Server received gossip event: {:?}", event);
43 |         if let Event::Gossip(GossipEvent::Received(msg)) = event {
44 |             println!("Server received message: {:?}", msg);
45 |         }
46 |     }
47 |     println!("Account loop ended");
48 |     Ok(())
49 | }
50 | 
51 | impl ProtocolHandler for Membership {
52 |     /// The returned future runs on a newly spawned tokio task, so it can run as long as
53 |     /// the connection lasts.
54 |     fn accept(&self, connecting: Connecting) -> BoxedFuture<Result<()>> {
55 |         let this = self.clone();
56 |         Box::pin(async move {
57 |             let connection = connecting.await?;
58 |             let device_id = get_remote_node_id(&connection)?;
59 |             println!("accepted connection from {device_id}");
60 | 
61 |             let account_id = this
62 |                 .db
63 |                 .get_device_account(device_id.into())?
64 |                 .ok_or(anyhow::anyhow!("unknown account"))?;
65 | 
66 |             println!("authenticated as {}", account_id);
67 | 
68 |             // fetch account's organizations
69 | 
70 |             let (mut send, mut recv) = connection.accept_bi().await?;
71 | 
72 |             println!("accepted connection");
73 | 
74 |             loop {
75 |                 let message = Message::read_async(&mut recv).await?;
76 |                 println!("got message");
77 |                 match message.get_type() {
78 |                     SignedMessage::TAG => {
79 |                         println!("got signed message");
80 | 
81 |                         let signed = message.decode::<SignedMessage>()?;
82 |                         if signed.verify()? != device_id.into() {
83 |                             anyhow::bail!("message must be signed by the device");
84 |                         }
85 | 
86 |                         match signed.get_message().get_type() {
87 |                             MembershipRequest::TAG => {
88 |                                 let request = signed.get_message().decode::<MembershipRequest>()?;
89 |                                 let device = request.device_ownership.device;
90 |                                 let account = request.device_ownership.account;
91 | 
92 |                                 this.db.insert_device_account(device, account)?;
93 |                                 println!("added device `{}` to account `{}`", device, account);
94 | 
95 |                                 let response = MembershipResponse::new(true, None);
96 |                                 let encoded = MembershipResponse::encode(&response)?;
97 |                                 let signed =
98 |                                     SignedMessage::sign(this.endpoint.secret_key(), encoded)?;
99 |                                 let encoded = SignedMessage::encode(&signed)?;
100 |                                 encoded.write_async(&mut send).await?;
101 |                             }
102 |                             _ => anyhow::bail!("unknown message type"),
103 |                         }
104 |                     }
105 |                     ServeMeRequest::TAG => {
106 |                         println!("got serve me request");
107 |                         let topic = TopicId::from_bytes(account_id.into());
108 | 
109 |                         println!("subscribing");
110 |                         let (sender, receiver) =
111 |                             this.gossip.subscribe(topic, vec![device_id])?.split();
112 | 
113 |                         println!("spawning account loop");
114 |                         let _handle = tokio::spawn(account_loop(sender, receiver));
115 |                         println!("spawned");
116 | 
117 |                         let response = ServeMeResponse {};
118 |                         let encoded = ServeMeResponse::encode(&response)?;
119 |                         encoded.write_async(&mut send).await?;
120 |                     }
121 |                     _ => anyhow::bail!("unknown message type"),
122 |                 }
123 |             }
124 |         })
125 |     }
126 | }
127 | 
128 | //
129 | 
130 | #[derive(Debug, Serialize, Deserialize)]
131 | pub struct DeviceOwnership {
132 |     account: AccId,
133 |     device: NodeId,
134 | }
135 | 
136 | #[derive(Debug, Serialize, Deserialize)]
137 | pub struct MembershipRequest {
138 |     device_ownership: DeviceOwnership,
139 |     org: OrgId,
140 | }
141 | 
142 | impl MessageType for MembershipRequest {
143 |     const TAG: u64 = MembershipRequest::TAG;
144 | }
145 | 
146 | impl MembershipRequest {
147 |     pub const TAG: u64 = 0x483A130AB92F3040;
148 | 
149 |     pub fn new(device: NodeId, account: AccId, org: OrgId) -> Self {
150 |         Self {
151 |             device_ownership: DeviceOwnership { account, device },
152 |             org,
153 |         }
154 |     }
155 | }
156 | 
157 | //
158 | 
159 | #[derive(Serialize, Deserialize)]
160 | pub struct MembershipResponse {
161 |     // request: SignedMessage,
162 |     accepted: bool,
163 |     expiration: Option<Timestamp>,
164 | }
165 | 
166 | impl MessageType for MembershipResponse {
167 |     const TAG: u64 = MembershipResponse::TAG;
168 | }
169 | 
170 | impl MembershipResponse {
171 |     pub const TAG: u64 = 0xE6DD0F88165F0752;
172 | 
173 |     pub fn new(accepted: bool, expiration: Option<Timestamp>) -> Self {
174 |         Self {
175 |             accepted,
176 |             expiration,
177 |         }
178 |     }
179 | }
180 | 
181 | //
182 | 
183 | #[derive(Debug, Serialize, Deserialize)]
184 | pub struct ServeMeRequest {}
185 | 
186 | impl MessageType for ServeMeRequest {
187 |     const TAG: u64 = ServeMeRequest::TAG;
188 | }
189 | 
190 | impl ServeMeRequest {
191 |     pub const TAG: u64 = 0xBA030E95BD57F286;
192 | }
193 | 
194 | #[derive(Debug, Serialize, Deserialize)]
195 | pub struct ServeMeResponse {}
196 | 
197 | impl MessageType for ServeMeResponse {
198 |     const TAG: u64 = ServeMeResponse::TAG;
199 | }
200 | 
201 | impl ServeMeResponse {
202 |     pub const TAG: u64 = 0x73D6A76A63E79C06;
203 | }
```

crates/huly/src/message.rs
```
1 | // Huly™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | 
3 | use crate::id::{Hash, PKey};
4 | use anyhow::Result;
5 | use bytes::{Buf, Bytes, BytesMut};
6 | use chrono::{DateTime, TimeZone, Utc};
7 | use ed25519_dalek::Signature;
8 | use iroh::{PublicKey, SecretKey};
9 | use serde::{de::DeserializeOwned, Deserialize, Serialize};
10 | use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufWriter};
11 | 
12 | type Tag = u64;
13 | type Format = u8;
14 | 
15 | const POSTCARD_FORMAT: Format = 0x00;
16 | 
17 | #[derive(Debug, Clone, Serialize, Deserialize)]
18 | enum Data {
19 |     Blob(Hash),
20 |     Inline(Bytes),
21 | }
22 | 
23 | impl Data {
24 |     pub fn decode<T>(&self) -> Result<T>
25 |     where
26 |         T: DeserializeOwned,
27 |     {
28 |         match &self {
29 |             Self::Inline(bytes) => postcard::from_bytes(bytes.as_ref()).map_err(Into::into),
30 |             Self::Blob(_) => Err(anyhow::anyhow!("blob decoding not implemented")),
31 |         }
32 |     }
33 | 
34 |     pub fn as_bytes(&self) -> &[u8] {
35 |         match self {
36 |             Self::Inline(bytes) => bytes.as_ref(),
37 |             Self::Blob(_) => panic!("blob decoding not implemented"),
38 |         }
39 |     }
40 | }
41 | 
42 | #[derive(Debug, Serialize, Deserialize)]
43 | pub struct Message {
44 |     message_type: Tag,
45 |     data_format: Format,
46 |     data: Data,
47 | }
48 | 
49 | impl Message {
50 |     const MAX_MESSAGE_SIZE: usize = 0x10000;
51 |     const HEADER_SIZE: usize = 10;
52 | 
53 |     pub async fn read_async(mut reader: impl AsyncRead + Unpin) -> Result<Self> {
54 |         let mut header = [0u8; Self::HEADER_SIZE];
55 |         reader.read_exact(&mut header).await?;
56 | 
57 |         let mut header = Bytes::copy_from_slice(&header);
58 |         let tag = header.get_u64_le();
59 |         let format = header.get_u8();
60 |         let is_inline = header.get_u8() == 0;
61 | 
62 |         if is_inline {
63 |             let size = reader.read_u16_le().await? as usize;
64 |             if size > Self::MAX_MESSAGE_SIZE {
65 |                 anyhow::bail!("Incoming message exceeds the maximum message size");
66 |             }
67 |             let mut buffer = BytesMut::with_capacity(size);
68 |             let mut remaining = size;
69 | 
70 |             while remaining > 0 {
71 |                 let r = reader.read_buf(&mut buffer).await?;
72 |                 if r == 0 {
73 |                     anyhow::bail!("Unexpected EOF");
74 |                 }
75 |                 remaining = remaining.saturating_sub(r);
76 |             }
77 |             Ok(Message {
78 |                 message_type: tag,
79 |                 data_format: format,
80 |                 data: Data::Inline(buffer.freeze()),
81 |             })
82 |         } else {
83 |             let mut hash: [u8; 32] = [0; 32];
84 |             let _ = reader.read_exact(&mut hash).await?;
85 |             Ok(Message {
86 |                 message_type: tag,
87 |                 data_format: format,
88 |                 data: Data::Blob(hash),
89 |             })
90 |         }
91 |     }
92 | 
93 |     pub async fn write_async(&self, writer: impl AsyncWrite + Unpin) -> Result<()> {
94 |         let mut writer = BufWriter::new(writer);
95 | 
96 |         writer.write_u64_le(self.message_type).await?;
97 |         writer.write_u8(self.data_format).await?;
98 |         match &self.data {
99 |             Data::Inline(bytes) => {
100 |                 writer.write_u8(0).await?;
101 |                 writer.write_u16_le(bytes.len() as u16).await?;
102 |                 writer.write_all(bytes).await?;
103 |             }
104 |             Data::Blob(hash) => {
105 |                 writer.write_u8(0xff).await?;
106 |                 writer.write_all(hash).await?;
107 |             }
108 |         }
109 | 
110 |         writer.flush().await?;
111 |         Ok(())
112 |     }
113 | 
114 |     pub fn decode<T>(&self) -> Result<T>
115 |     where
116 |         T: DeserializeOwned + MessageType,
117 |     {
118 |         if self.message_type != T::TAG {
119 |             Err(anyhow::anyhow!(
120 |                 "wrong message type, expected {} got {}",
121 |                 T::TAG,
122 |                 self.message_type
123 |             ))
124 |         } else {
125 |             self.data.decode::<T>()
126 |         }
127 |     }
128 | 
129 |     pub fn get_type(&self) -> Tag {
130 |         self.message_type
131 |     }
132 | 
133 |     pub fn get_payload<T>(&self) -> Result<T>
134 |     where
135 |         T: DeserializeOwned,
136 |     {
137 |         self.data.decode::<T>()
138 |     }
139 | }
140 | 
141 | pub trait MessageType: Serialize + DeserializeOwned {
142 |     const TAG: Tag;
143 | 
144 |     fn encode(&self) -> Result<Message> {
145 |         Ok(Message {
146 |             message_type: Self::TAG,
147 |             data_format: POSTCARD_FORMAT,
148 |             data: Data::Inline(postcard::to_stdvec(self)?.into()),
149 |         })
150 |     }
151 | }
152 | 
153 | // pub struct MessageType<T, const ID: Tag>
154 | // where
155 | //     T: Serialize + DeserializeOwned,
156 | // {
157 | //     _marker: std::marker::PhantomData<T>,
158 | // }
159 | 
160 | // impl<T, const ID: Tag> MessageType<T, ID>
161 | // where
162 | //     T: Serialize + DeserializeOwned,
163 | // {
164 | //     pub const TAG: Tag = ID;
165 | 
166 | //     pub fn encode(message: &T) -> Result<Message> {
167 | //         Ok(Message {
168 | //             message_type: Self::TAG,
169 | //             data_format: POSTCARD_FORMAT,
170 | //             data: Data::Inline(postcard::to_stdvec(message)?.into()),
171 | //         })
172 | //     }
173 | 
174 | //     pub fn decode(message: &Message) -> Result<T> {
175 | //         if message.get_type() != Self::TAG {
176 | //             Err(anyhow::anyhow!("unexpected message type"))
177 | //         } else {
178 | //             message.data.decode()
179 | //         }
180 | //     }
181 | // }
182 | 
183 | //
184 | 
185 | #[derive(Debug, Serialize, Deserialize)]
186 | pub struct SignedMessage {
187 |     message: Message,
188 |     by: PKey,
189 |     signature: Signature,
190 | }
191 | 
192 | impl MessageType for SignedMessage {
193 |     const TAG: Tag = SignedMessage::TAG;
194 | }
195 | 
196 | impl SignedMessage {
197 |     #[allow(clippy::unusual_byte_groupings)]
198 |     pub const TAG: Tag = 0x131C5_FACADE_699EA;
199 | 
200 |     pub fn sign(secret_key: &SecretKey, message: Message) -> Result<Self> {
201 |         let signature = secret_key.sign(message.data.as_bytes());
202 |         Ok(SignedMessage {
203 |             message,
204 |             signature,
205 |             by: secret_key.public().into(),
206 |         })
207 |     }
208 | 
209 |     pub fn verify(&self) -> Result<PKey> {
210 |         let key: PublicKey = self.by.into();
211 |         key.verify(self.message.data.as_bytes(), &self.signature)?;
212 |         Ok(self.by)
213 |     }
214 | 
215 |     pub fn get_message(&self) -> &Message {
216 |         &self.message
217 |     }
218 | }
219 | 
220 | // Timestamp
221 | 
222 | #[derive(Debug, Serialize, Deserialize)]
223 | pub struct Timestamp(i64);
224 | 
225 | impl From<DateTime<Utc>> for Timestamp {
226 |     fn from(dt: DateTime<Utc>) -> Self {
227 |         Timestamp(dt.timestamp())
228 |     }
229 | }
230 | 
231 | impl TryInto<DateTime<Utc>> for Timestamp {
232 |     type Error = anyhow::Error;
233 | 
234 |     fn try_into(self) -> Result<DateTime<Utc>> {
235 |         match Utc.timestamp_opt(self.0, 0) {
236 |             chrono::LocalResult::Single(datetime) => Ok(datetime),
237 |             chrono::LocalResult::None => anyhow::bail!("timestamp is out of range"),
238 |             chrono::LocalResult::Ambiguous(_, _) => anyhow::bail!("timestamp is ambiguous"),
239 |         }
240 |     }
241 | }
242 | 
243 | //
244 | 
245 | // async fn read_lp(
246 | //     mut reader: impl AsyncRead + Unpin,
247 | //     buffer: &mut BytesMut,
248 | //     max_message_size: usize,
249 | // ) -> Result<Option<Bytes>> {
250 | //     let size = match reader.read_u32().await {
251 | //         Ok(size) => size,
252 | //         Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
253 | //         Err(err) => return Err(err.into()),
254 | //     };
255 | //     let mut reader = reader.take(size as u64);
256 | //     let size = usize::try_from(size).context("frame larger than usize")?;
257 | //     if size > max_message_size {
258 | //         anyhow::bail!(
259 | //             "Incoming message exceeds the maximum message size of {max_message_size} bytes"
260 | //         );
261 | //     }
262 | //     buffer.reserve(size);
263 | //     loop {
264 | //         let r = reader.read_buf(buffer).await?;
265 | //         if r == 0 {
266 | //             break;
267 | //         }
268 | //     }
269 | //     Ok(Some(buffer.split_to(size).freeze()))
270 | // }
271 | 
272 | // async fn write_lp(
273 | //     mut writer: impl AsyncWrite + Unpin,
274 | //     buffer: &Bytes,
275 | //     max_message_size: usize,
276 | // ) -> Result<()> {
277 | //     let size = if buffer.len() > max_message_size {
278 | //         anyhow::bail!("message too large");
279 | //     } else {
280 | //         buffer.len() as u32
281 | //     };
282 | //     writer.write_u32(size).await?;
283 | //     writer.write_all(&buffer).await?;
284 | //     Ok(())
285 | // }
```

crates/huly-cli/src/console.rs
```
1 | // RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // main.rs:
4 | 
5 | use anyhow::Result;
6 | use colored::*;
7 | use rebeldb::eval::Context;
8 | use rebeldb::heap::TempHeap;
9 | use rebeldb::parser::ValueIterator;
10 | use rebeldb::value::Value;
11 | use rustyline::{error::ReadlineError, DefaultEditor};
12 | 
13 | fn evaluate(input: &str, heap: &mut TempHeap, ctx: &mut Context) -> Result<Value> {
14 |     ctx.read_all(ValueIterator::new(input, heap))?;
15 |     Ok(ctx.eval()?)
16 | }
17 | 
18 | fn main() -> Result<()> {
19 |     println!(
20 |         "{} © 2025 Huly Labs • {}",
21 |         "Huly Platform™".bold(),
22 |         "https://hulylabs.com".underline()
23 |     );
24 |     println!(
25 |         "{} console. Type {} or press Ctrl+D to exit\n",
26 |         "RebelDB™",
27 |         ":quit".red().bold()
28 |     );
29 | 
30 |     // Initialize interpreter
31 |     //
32 |     let mut blobs = TempHeap::new();
33 |     let mut ctx = Context::new();
34 |     ctx.load_module(&rebeldb::core::CORE_MODULE);
35 | 
36 |     // Setup rustyline editor
37 |     let mut rl = DefaultEditor::new()?;
38 | 
39 |     // Load history from previous sessions
40 |     // let history_path = PathBuf::from(".history");
41 |     // if rl.load_history(&history_path).is_err() {
42 |     //     println!("No previous history.");
43 |     // }
44 | 
45 |     loop {
46 |         let readline = rl.readline(&"RebelDB™ ❯ ".to_string());
47 |         // let readline = rl.readline(&"RebelDB™ • ".to_string());
48 | 
49 |         match readline {
50 |             Ok(line) => {
51 |                 // Add to history
52 |                 rl.add_history_entry(line.as_str())?;
53 | 
54 |                 // Handle special commands
55 |                 if line.trim() == ":quit" {
56 |                     break;
57 |                 }
58 | 
59 |                 match evaluate(&line, &mut blobs, &mut ctx) {
60 |                     Ok(result) => println!("{}:  {}", "OK".green(), result),
61 |                     Err(err) => eprintln!("{}: {}", "ERR".red().bold(), err),
62 |                 }
63 |             }
64 |             Err(ReadlineError::Interrupted) => {
65 |                 println!("CTRL-C");
66 |                 continue;
67 |             }
68 |             Err(ReadlineError::Eof) => {
69 |                 println!("CTRL-D");
70 |                 break;
71 |             }
72 |             Err(err) => {
73 |                 println!("Error: {:?}", err);
74 |                 break;
75 |             }
76 |         }
77 |     }
78 | 
79 |     // Save history
80 |     // rl.save_history(&history_path)?;
81 | 
82 |     Ok(())
83 | }
```

crates/huly-cli/src/main.rs
```
1 | //
2 | 
3 | use anyhow::{bail, Ok, Result};
4 | use clap::Parser;
5 | use config::Config;
6 | use huly::db::Db;
7 | use huly::id::{AccId, OrgId};
8 | use huly::membership::Membership;
9 | use iroh::protocol::Router;
10 | use iroh::{Endpoint, RelayMap, RelayMode, RelayUrl, SecretKey};
11 | use iroh_gossip::net::{Gossip, GOSSIP_ALPN};
12 | use std::net::{Ipv4Addr, SocketAddrV4};
13 | 
14 | /// By default, the relay server run by n0 is used. To use a local relay server, run
15 | ///     cargo run --bin iroh-relay --features iroh-relay -- --dev
16 | /// in another terminal and then set the `-d http://localhost:3340` flag on this example.
17 | #[derive(Parser, Debug)]
18 | struct Args {
19 |     // #[clap(long)]
20 |     // secret_key: Option<String>,
21 |     #[clap(short, long)]
22 |     relay: Option<RelayUrl>,
23 |     #[clap(long)]
24 |     no_relay: bool,
25 |     #[clap(short, long)]
26 |     db: String,
27 |     #[clap(long)]
28 |     db_init: bool,
29 |     #[clap(short, long, default_value = "0")]
30 |     bind_port: u16,
31 |     #[clap(subcommand)]
32 |     command: Command,
33 | }
34 | 
35 | #[derive(Parser, Debug)]
36 | enum Command {
37 |     Client { server: String, account: String },
38 |     Server {},
39 |     CreateDb,
40 | }
41 | 
42 | #[tokio::main]
43 | async fn main() -> Result<()> {
44 |     tracing_subscriber::fmt::init();
45 |     let args = Args::parse();
46 | 
47 |     let settings = Config::builder()
48 |         // .add_source(config::File::with_name("settings"))
49 |         .add_source(config::Environment::with_prefix("HULY"))
50 |         .build()
51 |         .unwrap();
52 | 
53 |     let secret_key = match settings.get::<Option<String>>("secret")? {
54 |         None => SecretKey::generate(rand::rngs::OsRng),
55 |         Some(key) => key.parse()?,
56 |     };
57 | 
58 |     println!("secret: {}", secret_key);
59 | 
60 |     // configure relay map
61 |     let relay_mode = match (args.no_relay, args.relay) {
62 |         (false, None) => RelayMode::Default,
63 |         (false, Some(url)) => RelayMode::Custom(RelayMap::from_url(url)),
64 |         (true, None) => RelayMode::Disabled,
65 |         (true, Some(_)) => bail!("You cannot set --no-relay and --relay at the same time"),
66 |     };
67 | 
68 |     println!("using secret key: {secret_key}");
69 |     println!("using relay servers: {}", fmt_relay_mode(&relay_mode));
70 | 
71 |     let endpoint = Endpoint::builder()
72 |         .secret_key(secret_key.clone())
73 |         .relay_mode(relay_mode)
74 |         .bind_addr_v4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, args.bind_port))
75 |         .discovery_local_network()
76 |         // .discovery_dht()
77 |         // .discovery_n0()
78 |         .bind()
79 |         .await?;
80 | 
81 |     println!("ready with node id: {}", endpoint.node_id());
82 | 
83 |     let db = match args.db_init {
84 |         true => Db::create(&args.db)?,
85 |         false => Db::open(&args.db)?,
86 |     };
87 | 
88 |     let router_builder = Router::builder(endpoint.clone());
89 | 
90 |     let gossip = Gossip::builder().spawn(endpoint.clone()).await?;
91 |     let router_builder = router_builder.accept(GOSSIP_ALPN, gossip.clone());
92 | 
93 |     let membership = Membership::new(db, endpoint.clone(), gossip.clone());
94 |     let router_builder = router_builder.accept(Membership::ALPN, membership.clone());
95 | 
96 |     let router = router_builder.spawn().await?;
97 | 
98 |     match args.command {
99 |         Command::Server {} => {
100 |             let node_id = router.endpoint().node_id();
101 |             println!("membership proto started on node id: {node_id}");
102 | 
103 |             // for text in text.into_iter() {
104 |             //     proto.insert_and_index(text).await?;
105 |             // }
106 | 
107 |             // Wait for Ctrl-C to be pressed.
108 |             tokio::signal::ctrl_c().await?;
109 |         }
110 |         Command::Client { server, account } => {
111 |             let account: AccId = account.parse()?;
112 |             let org: OrgId = server.parse()?;
113 |             huly::client::request_membership(
114 |                 &secret_key.clone(),
115 |                 endpoint.clone(),
116 |                 account,
117 |                 org,
118 |                 gossip,
119 |             )
120 |             .await?;
121 |         }
122 |         Command::CreateDb => {
123 |             let _ = Db::create(&args.db)?;
124 |         }
125 |     }
126 | 
127 |     // sleep(Duration::from_secs(60)).await;
128 | 
129 |     // 88877a049601655b479cf46b906669266066a6eda2473aadf1574fffaa1353a7
130 |     // 67c78c9886bc71fd91415577e078de03966bc17603d52a1355ad53cb53571ae1
131 | 
132 |     // 802ec3ff23cdd6bc67b4b45c9d3dd92bd518c1b4c6708fcde1ce2a1a7abc6aef
133 |     // b60988059e237d6e1ccc9f1b9985123a3db34b21a527e14b4bad99574aeabed9
134 | 
135 |     // Account:
136 |     // d28aeaafe8e8c70f16bc862085795dfcb45c083ab8ff0754654b0e35a45fe339
137 |     // 22cfbf283eb134a3cde229fec9de9f97aa946021d484e66a308b7a79b005c814
138 | 
139 |     // let peers: Vec<PublicKey> = vec![
140 |     //     "67c78c9886bc71fd91415577e078de03966bc17603d52a1355ad53cb53571ae1".parse()?,
141 |     //     "b60988059e237d6e1ccc9f1b9985123a3db34b21a527e14b4bad99574aeabed9".parse()?,
142 |     //     "22cfbf283eb134a3cde229fec9de9f97aa946021d484e66a308b7a79b005c814".parse()?,
143 |     // ];
144 | 
145 |     // run(endpoint, peers).await
146 |     // let client = Client::connect(
147 |     //     uuid::Uuid::new_v4(),
148 |     //     secret_key,
149 |     //     vec![],
150 |     //     relay_mode,
151 |     //     args.bind_port,
152 |     // )
153 |     // .await?;
154 | 
155 |     // client.run().await
156 | 
157 |     router.shutdown().await?;
158 | 
159 |     Ok(())
160 | }
161 | 
162 | fn fmt_relay_mode(relay_mode: &RelayMode) -> String {
163 |     match relay_mode {
164 |         RelayMode::Disabled => "None".to_string(),
165 |         RelayMode::Default => "Default Relay (production) servers".to_string(),
166 |         RelayMode::Staging => "Default Relay (staging) servers".to_string(),
167 |         RelayMode::Custom(map) => map
168 |             .urls()
169 |             .map(|url| url.to_string())
170 |             .collect::<Vec<_>>()
171 |             .join(", "),
172 |     }
173 | }
```

crates/rebeldb/build.rs
```
1 | // RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // build.rs:
4 | 
5 | use std::env;
6 | use std::path::Path;
7 | use std::process::Command;
8 | 
9 | fn main() {
10 |     // Watch for changes in runtime dependencies
11 |     println!("cargo:rerun-if-changed=../rebeldb-runtime/src");
12 |     println!("cargo:rerun-if-changed=../rebeldb-runtime/Cargo.toml");
13 |     println!("cargo:rerun-if-changed=build.rs");
14 | 
15 |     // Path to runtime-wasm crate - adjusted for crates/ directory
16 |     let wasm_runtime_crate = Path::new("../rebeldb-runtime");
17 | 
18 |     // Build runtime-wasm
19 |     let status = Command::new("cargo")
20 |         .current_dir(wasm_runtime_crate)
21 |         .args(["build", "--target", "wasm32-unknown-unknown", "--release"])
22 |         .status()
23 |         .expect("Failed to build wasm runtime");
24 | 
25 |     if !status.success() {
26 |         panic!("Failed to build wasm runtime");
27 |     }
28 | 
29 |     // Get workspace target directory
30 |     let target_dir = if let Ok(target) = env::var("CARGO_TARGET_DIR") {
31 |         Path::new(&target).to_path_buf()
32 |     } else {
33 |         // Adjusted to look for target in root, not crates/
34 |         Path::new("../../target").to_path_buf()
35 |     };
36 | 
37 |     // Setup paths
38 |     let wasm_file = target_dir.join("wasm32-unknown-unknown/release/rebeldb_runtime.wasm");
39 |     let assets_dir = Path::new("assets");
40 | 
41 |     // Create assets directory
42 |     std::fs::create_dir_all(assets_dir).expect("Failed to create assets directory");
43 | 
44 |     // Copy WASM file
45 |     let dest_path = assets_dir.join("rebeldb_runtime.wasm");
46 |     std::fs::copy(&wasm_file, &dest_path).expect("Failed to copy wasm file");
47 | 
48 |     // Watch the output WASM file for changes
49 |     println!("cargo:rerun-if-changed={}", wasm_file.display());
50 | }
```

crates/rebeldb/src/blob.rs
```
1 | // RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // blob.rs:
4 | 
5 | use crate::core::Hash;
6 | use bytes::Bytes;
7 | use std::collections::HashMap;
8 | use std::fmt;
9 | 
10 | pub trait Blobs {
11 |     fn get(&self, key: &Hash) -> Option<Bytes>;
12 |     fn put(&mut self, data: &[u8]) -> Hash;
13 | }
14 | 
15 | pub struct MemoryBlobs {
16 |     blobs: HashMap<Hash, Vec<u8>>,
17 | }
18 | 
19 | impl Default for MemoryBlobs {
20 |     fn default() -> Self {
21 |         Self::new()
22 |     }
23 | }
24 | 
25 | impl MemoryBlobs {
26 |     pub fn new() -> Self {
27 |         Self {
28 |             blobs: HashMap::new(),
29 |         }
30 |     }
31 | }
32 | 
33 | impl Blobs for MemoryBlobs {
34 |     fn get(&self, key: &Hash) -> Option<Bytes> {
35 |         self.blobs
36 |             .get(key)
37 |             .map(|v| Bytes::copy_from_slice(v.as_slice()))
38 |     }
39 | 
40 |     fn put(&mut self, data: &[u8]) -> Hash {
41 |         let hash = *blake3::hash(data).as_bytes();
42 |         self.blobs.insert(hash, data.to_vec());
43 |         hash
44 |     }
45 | }
46 | 
47 | impl fmt::Debug for MemoryBlobs {
48 |     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
49 |         writeln!(f, "  blobs: {{")?;
50 |         for (hash, bytes) in &self.blobs {
51 |             writeln!(f, "    {} =>", hex::encode(hash))?;
52 |             // Hexdump-like format, 16 bytes per line
53 |             for chunk in bytes.chunks(16) {
54 |                 // Hex part
55 |                 write!(f, "      ")?;
56 |                 for b in chunk {
57 |                     write!(f, "{:02x} ", b)?;
58 |                 }
59 |                 // Padding for incomplete last line
60 |                 for _ in chunk.len()..16 {
61 |                     write!(f, "   ")?;
62 |                 }
63 |                 // ASCII part
64 |                 write!(f, " |")?;
65 |                 for &b in chunk {
66 |                     let c = if b.is_ascii_graphic() || b == b' ' {
67 |                         b as char
68 |                     } else {
69 |                         '.'
70 |                     };
71 |                     write!(f, "{}", c)?;
72 |                 }
73 |                 // Padding for incomplete last line
74 |                 for _ in chunk.len()..16 {
75 |                     write!(f, " ")?;
76 |                 }
77 |                 writeln!(f, "|")?;
78 |             }
79 |         }
80 |         writeln!(f, "  }}")
81 |     }
82 | }
```

crates/rebeldb/src/block.rs
```
1 | // RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // block.rs:
4 | 
5 | use crate::blob::Blobs;
6 | use crate::core::{Content, Hash, InlineBytes, Value};
7 | use bytes::{BufMut, BytesMut};
8 | 
9 | pub struct BlockBuilder {
10 |     bytes: BytesMut,
11 |     offsets: Vec<u32>,
12 | }
13 | 
14 | impl BlockBuilder {
15 |     pub fn new() -> Self {
16 |         Self {
17 |             bytes: BytesMut::new(),
18 |             offsets: Vec::new(),
19 |         }
20 |     }
21 | 
22 |     pub fn uint(&mut self, v: u32) {
23 |         self.bytes.put_u8(UINT_TAG);
24 |         self.bytes.put_u32_le(v);
25 |         self.offsets.push(self.bytes.len() as u32);
26 |     }
27 | 
28 |     pub fn float(&mut self, v: f32) {
29 |         self.bytes.put_u8(FLOAT_TAG);
30 |         self.bytes.put_f32_le(v);
31 |         self.offsets.push(self.bytes.len() as u32);
32 |     }
33 | 
34 |     pub fn string(&mut self, blobs: &mut impl Blobs, v: &str) {
35 |         self.bytes.put_u8(STRING_TAG);
36 |         if v.len() <= std::mem::size_of::<InlineBytes>() {
37 |             self.bytes.put_u8(v.len() as u8);
38 |             self.bytes.put(v.as_bytes());
39 |         } else {
40 |             let hash = blobs.put(v.as_bytes());
41 |             self.bytes.put_u8(HASH_TAG);
42 |             self.bytes.put(hash.as_slice());
43 |         }
44 |         self.offsets.push(self.bytes.len() as u32);
45 |     }
46 | 
47 |     pub fn build(&mut self) -> Block {
48 |         for offset in self.offsets.iter().rev() {
49 |             self.bytes.put_u32_le(*offset);
50 |         }
51 |         self.bytes.put_u32_le(self.offsets.len() as u32);
52 |         Block {
53 |             bytes: &self.bytes, //.clone().freeze(),
54 |         }
55 |     }
56 | }
57 | 
58 | fn read_u32(bytes: &[u8], offset: usize) -> usize {
59 |     let b0 = bytes[offset] as usize;
60 |     let b1 = bytes[offset + 1] as usize;
61 |     let b2 = bytes[offset + 2] as usize;
62 |     let b3 = bytes[offset + 3] as usize;
63 | 
64 |     b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
65 | }
66 | 
67 | pub struct Block<'a> {
68 |     bytes: &'a [u8],
69 | }
70 | 
71 | impl<'a> Block<'a> {
72 |     pub fn new(bytes: &'a [u8]) -> Self {
73 |         Self { bytes }
74 |     }
75 | 
76 |     pub fn len(&self) -> Option<usize> {
77 |         let size = self.bytes.len();
78 |         if size < std::mem::size_of::<u32>() {
79 |             None
80 |         } else {
81 |             Some(read_u32(self.bytes, size - 4))
82 |         }
83 |     }
84 | 
85 |     pub fn get(&self, index: usize) -> Option<Value> {
86 |         if let Some(item) = self.get_item(index) {
87 |             let tag = item[0];
88 |             match tag {
89 |                 NONE_TAG => Some(Value::None),
90 |                 UINT_TAG => {
91 |                     let mut buf: [u8; 4] = [0; 4];
92 |                     buf.copy_from_slice(&item[1..5]);
93 |                     Some(Value::Uint(u32::from_le_bytes(buf)))
94 |                 }
95 |                 INT_TAG => {
96 |                     let mut buf: [u8; 4] = [0; 4];
97 |                     buf.copy_from_slice(&item[1..5]);
98 |                     Some(Value::Int(i32::from_le_bytes(buf)))
99 |                 }
100 |                 FLOAT_TAG => {
101 |                     let mut buf: [u8; 4] = [0; 4];
102 |                     buf.copy_from_slice(&item[1..5]);
103 |                     Some(Value::Float(f32::from_le_bytes(buf)))
104 |                 }
105 |                 STRING_TAG => {
106 |                     let mut tag: [u8; 1] = [0; 1];
107 |                     tag.copy_from_slice(&item[1..2]);
108 |                     if tag[0] == HASH_TAG {
109 |                         let mut hash: Hash = [0; 32];
110 |                         hash.copy_from_slice(&item[2..34]);
111 |                         Some(Value::String(Content::Hash(hash)))
112 |                     } else {
113 |                         let len = tag[0] as usize;
114 |                         let mut buf: InlineBytes = [0; 37];
115 |                         buf[..len].copy_from_slice(&item[2..2 + len]);
116 |                         Some(Value::String(Content::Inline((tag[0], buf))))
117 |                     }
118 |                 }
119 |                 _ => None,
120 |             }
121 |         } else {
122 |             None
123 |         }
124 |     }
125 | 
126 |     fn get_item(&self, index: usize) -> Option<&[u8]> {
127 |         let len = self.bytes.len();
128 |         if let Some(count_offset) = len.checked_sub(std::mem::size_of::<u32>()) {
129 |             let count = read_u32(self.bytes, count_offset) as usize;
130 |             if index < count {
131 |                 if let Some(end) = count_offset.checked_sub(4 * (index + 1)) {
132 |                     let end_offset = read_u32(self.bytes, end) as usize;
133 |                     let start_offset = if index == 0 {
134 |                         0
135 |                     } else {
136 |                         read_u32(self.bytes, end + 4) as usize
137 |                     };
138 |                     return Some(&self.bytes[start_offset..end_offset]);
139 |                 }
140 |             }
141 |         }
142 |         None
143 |     }
144 | }
145 | 
146 | pub struct BlockIterator<'a> {
147 |     block: &'a [u8],
148 |     count: usize,
149 |     position: usize,
150 |     offset: usize,
151 | }
152 | 
153 | impl<'a> BlockIterator<'a> {
154 |     pub fn new(block: &'a [u8]) -> Option<Self> {
155 |         let len = block.len();
156 |         if let Some(count_offset) = len.checked_sub(std::mem::size_of::<u32>()) {
157 |             let count = read_u32(block, count_offset) as usize;
158 |             Some(Self {
159 |                 block,
160 |                 count,
161 |                 offset: 0,
162 |                 position: 0,
163 |             })
164 |         } else {
165 |             None
166 |         }
167 |     }
168 | }
169 | 
170 | impl<'a> Iterator for BlockIterator<'a> {
171 |     type Item = Value;
172 | 
173 |     fn next(&mut self) -> Option<Self::Item> {
174 |         if self.position < self.count {
175 |             if let Some(end) = self.block.len().checked_sub(4 * (self.position + 2)) {
176 |                 let end_offset = read_u32(self.block, end) as usize;
177 |                 let slice = &self.block[self.offset..end_offset];
178 |                 self.offset = end_offset;
179 |                 self.position += 1;
180 |                 Some(&self.block[start_offset..end_offset])
181 |             } else {
182 |                 None
183 |             }
184 |         } else {
185 |             None
186 |         }
187 |     }
188 | }
189 | 
190 | #[cfg(test)]
191 | mod tests {
192 |     use super::*;
193 |     use anyhow::Result;
194 | 
195 |     struct NullBlobs;
196 | 
197 |     impl Blobs for NullBlobs {
198 |         fn get(&self, _key: &Hash) -> Option<Bytes> {
199 |             unreachable!()
200 |         }
201 | 
202 |         fn put(&mut self, _data: &[u8]) -> Hash {
203 |             unreachable!()
204 |         }
205 |     }
206 | 
207 |     #[test]
208 |     fn test_block_builder() -> Result<()> {
209 |         let mut blobs = NullBlobs {};
210 |         let mut builder = BlockBuilder::new();
211 |         builder.uint(199);
212 |         builder.float(3.14);
213 |         builder.string(&mut blobs, "hello world");
214 |         builder.uint(55);
215 |         let block = builder.build();
216 | 
217 |         // assert_eq!(block.len()?, 3);
218 | 
219 |         println!("{:?}", block.get(0));
220 |         println!("{:?}", block.get(1));
221 |         println!("{:?}", block.get(2));
222 |         println!("{:?}", block.get(3));
223 | 
224 |         Ok(())
225 |     }
226 | }
```

crates/rebeldb/src/boxed.rs
```
1 | #[derive(Clone, Copy, Debug, PartialEq, Eq)]
2 | pub struct BoxedValue(u64);
3 | 
4 | // We force exponent=0x7FF => bits 62..52
5 | const EXP_SHIFT: u64 = 52;
6 | const EXP_MAX: u64 = 0x7FF;
7 | const EXP_MASK: u64 = EXP_MAX << EXP_SHIFT; // bits 62..52 = all ones
8 | 
9 | // We'll always set fraction bit 51 = 1, so fraction != 0 => guaranteed NaN.
10 | const FRACTION_TOP_BIT: u64 = 1 << 51; // 0x8000_0000_0000
11 | 
12 | // 4-bit tag in bits 50..47
13 | const TAG_SHIFT: u64 = 47;
14 | const TAG_MASK: u64 = 0xF;
15 | 
16 | // That leaves bits 46..0 (47 bits) for the payload.
17 | const PAYLOAD_MASK_47: u64 = (1 << 47) - 1; // 0x7FFF_FFFF_FFFF
18 | 
19 | /// Example tags
20 | #[repr(u64)]
21 | #[derive(Clone, Copy, Debug, PartialEq, Eq)]
22 | enum Tag {
23 |     Int = 0x0, // up to you which nibble you choose
24 |     Ptr = 0x1,
25 |     // up to 0xF ...
26 | }
27 | 
28 | impl BoxedValue {
29 |     /// Create a boxed *signed* integer with 47-bit 2's complement payload.
30 |     ///
31 |     /// Valid range: -2^46 .. 2^46 - 1
32 |     /// (i.e. about ±140.7 trillion)
33 |     pub fn new_int(value: i64) -> Self {
34 |         // Check range
35 |         let min = -(1 << 46); // -140,737,488,355,328
36 |         let max = (1 << 46) - 1; // +140,737,488,355,327
37 |         assert!(value >= min && value <= max, "Integer out of 47-bit range");
38 | 
39 |         // We want to store this i64 in the low 47 bits, 2's complement.
40 |         // Easiest approach is to mask off lower 47 bits of the sign-extended i64.
41 |         // 1) shift left 17, then arithmetic right 17 => sign-extend from bit 46
42 |         // 2) cast to u64 => the bottom 47 bits contain the 2's complement form
43 |         let payload_47 = ((value << (64 - 47)) >> (64 - 47)) as u64 & PAYLOAD_MASK_47;
44 | 
45 |         // Build fraction:
46 |         //   bit 51 = 1
47 |         //   bits 50..47 = Tag::Int
48 |         //   bits 46..0 = payload_47
49 |         let fraction = FRACTION_TOP_BIT | ((Tag::Int as u64) & TAG_MASK) << TAG_SHIFT | payload_47;
50 | 
51 |         // sign bit (63) = 0, exponent=0x7FF, fraction
52 |         let bits = (0 << 63) | EXP_MASK | fraction;
53 |         BoxedValue(bits)
54 |     }
55 | 
56 |     /// Interpret this BoxedValue as a 47-bit signed integer.
57 |     pub fn as_int(&self) -> i64 {
58 |         let bits = self.0;
59 | 
60 |         // 1) Check exponent is 0x7FF
61 |         let exponent = (bits >> EXP_SHIFT) & 0x7FF;
62 |         assert_eq!(
63 |             exponent, EXP_MAX,
64 |             "Not a NaN exponent, can't be a NaN-boxed value."
65 |         );
66 | 
67 |         // 2) Extract fraction
68 |         let fraction = bits & ((1 << 52) - 1); // lower 52 bits
69 | 
70 |         // bit 51 must be 1
71 |         assert!(
72 |             (fraction >> 51) == 1,
73 |             "Fraction bit 51 not set => Infinity or normal float."
74 |         );
75 | 
76 |         // 3) Check tag
77 |         let tag = (fraction >> TAG_SHIFT) & TAG_MASK;
78 |         assert_eq!(tag, Tag::Int as u64, "Tag != Int");
79 | 
80 |         // 4) Extract the 47-bit payload, sign-extend from bit 46
81 |         let payload_47 = fraction & PAYLOAD_MASK_47;
82 | 
83 |         // sign-extend from bit 46 => shift up, then arithmetic shift down
84 |         let shifted = (payload_47 << (64 - 47)) as i64; // cast to i64 => preserve bits
85 |         let value = shifted >> (64 - 47); // arithmetic shift right
86 |         value
87 |     }
88 | 
89 |     /// Create a boxed pointer (32 bits). Tag = Ptr, fraction bit 51=1, payload in bits 46..0.
90 |     pub fn new_ptr(addr: u32) -> Self {
91 |         let payload_47 = addr as u64; // zero-extended into 64
92 |                                       // We could store a 46- or 47-bit pointer, but typically 32 bits is enough.
93 | 
94 |         let fraction = FRACTION_TOP_BIT
95 |             | ((Tag::Ptr as u64) & TAG_MASK) << TAG_SHIFT
96 |             | (payload_47 & PAYLOAD_MASK_47);
97 | 
98 |         let bits = (0 << 63) | EXP_MASK | fraction;
99 |         BoxedValue(bits)
100 |     }
101 | 
102 |     /// Return the pointer as 32 bits.
103 |     pub fn as_ptr(&self) -> u32 {
104 |         let bits = self.0;
105 | 
106 |         // exponent must be 0x7FF
107 |         let exponent = (bits >> EXP_SHIFT) & 0x7FF;
108 |         assert_eq!(
109 |             exponent, EXP_MAX,
110 |             "Not a NaN exponent => not a NaN-boxed value."
111 |         );
112 | 
113 |         let fraction = bits & ((1 << 52) - 1);
114 |         // bit 51 must be 1 => otherwise Infinity or normal float
115 |         assert!(
116 |             (fraction >> 51) == 1,
117 |             "Fraction bit 51 not set => Infinity or normal float."
118 |         );
119 | 
120 |         let tag = (fraction >> TAG_SHIFT) & TAG_MASK;
121 |         assert_eq!(tag, Tag::Ptr as u64, "Tag != Ptr");
122 | 
123 |         // Just the lower 47 bits
124 |         let payload_47 = fraction & PAYLOAD_MASK_47;
125 |         payload_47 as u32
126 |     }
127 | 
128 |     /// Raw bits for debugging or advanced usage
129 |     pub fn bits(&self) -> u64 {
130 |         self.0
131 |     }
132 | }
133 | 
134 | #[cfg(test)]
135 | mod tests {
136 |     use super::*;
137 | 
138 |     #[test]
139 |     fn test_int_round_trip() {
140 |         let vals = [
141 |             0,
142 |             1,
143 |             -1,
144 |             42,
145 |             -42,
146 |             123_456_789,
147 |             -123_456_789,
148 |             (1 << 46) - 1, //  140,737,488,355,327
149 |             -(1 << 46),    // -140,737,488,355,328
150 |         ];
151 | 
152 |         for &v in &vals {
153 |             let b = BoxedValue::new_int(v);
154 |             let back = b.as_int();
155 |             assert_eq!(
156 |                 v,
157 |                 back,
158 |                 "Failed round-trip for {} => bits=0x{:016X} => {}",
159 |                 v,
160 |                 b.bits(),
161 |                 back
162 |             );
163 |         }
164 |     }
165 | 
166 |     #[test]
167 |     #[should_panic]
168 |     #[allow(arithmetic_overflow)]
169 |     fn test_int_out_of_range() {
170 |         // +2^46 is out of range: 140,737,488,355,328
171 |         BoxedValue::new_int((1 << 46) as i64);
172 |     }
173 | 
174 |     #[test]
175 |     fn test_ptr_round_trip() {
176 |         let ptrs = [0u32, 1, 0xDEAD_BEEF, 0xFFFF_FFFF];
177 |         for &p in &ptrs {
178 |             let b = BoxedValue::new_ptr(p);
179 |             let back = b.as_ptr();
180 |             assert_eq!(
181 |                 p,
182 |                 back,
183 |                 "Failed round-trip for pointer 0x{:08X} => bits=0x{:016X} => 0x{:08X}",
184 |                 p,
185 |                 b.bits(),
186 |                 back
187 |             );
188 |         }
189 |     }
190 | }
```

crates/rebeldb/src/codegen.rs
```
1 | // RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // codegen.rs:
4 | 
5 | use crate::parser::ValueIterator;
6 | use crate::value::Value;
7 | use crate::{heap::Heap, value::Serialize};
8 | use thiserror::Error;
9 | use wasm_encoder::{
10 |     CodeSection, Function, FunctionSection, Instruction, Module, TypeSection, ValType,
11 | };
12 | 
13 | #[derive(Debug, Error)]
14 | pub enum CompileError {
15 |     #[error(transparent)]
16 |     ParseError(#[from] crate::parser::ParseError),
17 |     #[error(transparent)]
18 |     ValueError(#[from] crate::value::ValueError),
19 | }
20 | 
21 | struct ConstantPool {
22 |     offset: usize,
23 |     data: Vec<u8>,
24 | }
25 | 
26 | impl ConstantPool {
27 |     fn new() -> Self {
28 |         Self {
29 |             offset: 0,
30 |             data: Vec::new(),
31 |         }
32 |     }
33 | 
34 |     fn add_value(&mut self, value: Value) -> Result<i32, CompileError> {
35 |         let offset = self.offset;
36 |         let value_size = value.serialize(&mut self.data)?;
37 |         self.offset += value_size;
38 |         Ok(offset as i32)
39 |     }
40 | }
41 | 
42 | pub struct Compiler {
43 |     module: Module,
44 |     types: TypeSection,
45 |     functions: FunctionSection,
46 |     codes: CodeSection,
47 |     constants: ConstantPool,
48 | }
49 | 
50 | impl Compiler {
51 |     const CONSTANTS_START: i32 = 0x1000;
52 | 
53 |     pub fn new() -> Self {
54 |         Self {
55 |             module: Module::new(),
56 |             types: TypeSection::new(),
57 |             functions: FunctionSection::new(),
58 |             codes: CodeSection::new(),
59 |             constants: ConstantPool::new(),
60 |         }
61 |     }
62 | 
63 |     pub fn make_function<T>(
64 |         &mut self,
65 |         params: Vec<ValType>,
66 |         results: Vec<ValType>,
67 |         body: ValueIterator<'_, T>,
68 |     ) -> Result<(), CompileError>
69 |     where
70 |         T: Heap,
71 |     {
72 |         //self.types.ty().function(params, results);
73 | 
74 |         let locals = vec![];
75 |         let mut func = Function::new(locals);
76 | 
77 |         for value in body {
78 |             let value = value?;
79 |             match value {
80 |                 Value::Int(i) => func.instruction(&Instruction::I64Const(i)),
81 |                 Value::Float(f) => func.instruction(&Instruction::F64Const(f)),
82 |                 Value::Bytes(enc, content) => {
83 |                     let offset = self.constants.add_value(Value::Bytes(enc, content))?;
84 |                     func.instruction(&Instruction::I32Const(Self::CONSTANTS_START + offset))
85 |                 }
86 |                 _ => unimplemented!(),
87 |             };
88 |         }
89 | 
90 |         Ok(())
91 |     }
92 | }
```

crates/rebeldb/src/core.rs
```
1 | // RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // core.rs:
4 | 
5 | use crate::eval::{EvalError, Module};
6 | use crate::value::Value;
7 | 
8 | fn add(stack: &mut Vec<Value>) -> Result<(), EvalError> {
9 |     let b = stack.pop().ok_or(EvalError::ArityMismatch(2, 0))?;
10 |     let a = stack.pop().ok_or(EvalError::ArityMismatch(2, 1))?;
11 | 
12 |     let result = match (a, b) {
13 |         (Value::Int(a), Value::Int(b)) => Value::Int(a + b),
14 |         _ => return Err(EvalError::MismatchedType),
15 |     };
16 | 
17 |     stack.push(result);
18 |     Ok(())
19 | }
20 | 
21 | pub const CORE_MODULE: Module = Module {
22 |     procs: &[("add", add)],
23 | };
```

crates/rebeldb/src/eval.rs
```
1 | // RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // eval.rs:
4 | 
5 | use crate::heap::Heap;
6 | use crate::parser::ValueIterator;
7 | use crate::value::{Symbol, Value};
8 | use std::collections::HashMap;
9 | use std::result::Result;
10 | use thiserror::Error;
11 | 
12 | #[derive(Debug, Error)]
13 | pub enum EvalError {
14 |     #[error("word not found: {0:?}")]
15 |     WordNotFound(Symbol),
16 |     #[error("mismatched type")]
17 |     MismatchedType,
18 |     #[error("not enough arguments")]
19 |     NotEnoughArgs,
20 |     #[error(transparent)]
21 |     ParseError(#[from] crate::parser::ParseError),
22 |     #[error(transparent)]
23 |     ValueError(#[from] crate::value::ValueError),
24 |     #[error("arity mismatch: expecting {0} parameters, provided {1}")]
25 |     ArityMismatch(usize, usize),
26 |     #[error("stack underflow")]
27 |     StackUnderflow,
28 | }
29 | 
30 | pub struct Module {
31 |     pub procs: &'static [(&'static str, NativeFn)],
32 | }
33 | 
34 | pub struct Context {
35 |     stack: Vec<Value>,
36 |     op_stack: Vec<NativeFn>,
37 |     env: HashMap<Symbol, Value>,
38 |     modules: Vec<Vec<NativeFn>>,
39 | }
40 | 
41 | impl Default for Context {
42 |     fn default() -> Self {
43 |         Self::new()
44 |     }
45 | }
46 | 
47 | impl Context {
48 |     pub fn new() -> Self {
49 |         Self {
50 |             stack: Vec::new(),
51 |             op_stack: Vec::new(),
52 |             env: HashMap::new(),
53 |             modules: Vec::new(),
54 |         }
55 |     }
56 | 
57 |     pub fn load_module(&mut self, module: &Module) {
58 |         let module_id = self.modules.len();
59 |         let mut procs: Vec<NativeFn> = Vec::new();
60 | 
61 |         for (id, proc) in module.procs.iter().enumerate() {
62 |             procs.push(proc.1);
63 |             let native_fn = Value::NativeFn(module_id, id);
64 |             self.ctx_put(Symbol::new(proc.0).unwrap(), native_fn);
65 |         }
66 |         self.modules.push(procs);
67 |     }
68 | 
69 |     pub fn push(&mut self, value: Value) {
70 |         match value {
71 |             Value::NativeFn(module, proc) => self.op_stack.push(self.modules[module][proc]),
72 |             _ => self.stack.push(value),
73 |         }
74 |     }
75 | 
76 |     pub fn pop(&mut self) -> Option<Value> {
77 |         self.stack.pop()
78 |     }
79 | 
80 |     pub fn ctx_put(&mut self, symbol: Symbol, value: Value) {
81 |         self.env.insert(symbol, value);
82 |     }
83 | 
84 |     pub fn read(&mut self, value: Value) -> Result<(), EvalError> {
85 |         match value {
86 |             Value::Word(word) => {
87 |                 if let Some(value) = self.env.get(&word) {
88 |                     self.push(value.clone());
89 |                     Ok(())
90 |                 } else {
91 |                     Err(EvalError::WordNotFound(word))
92 |                 }
93 |             }
94 |             _ => {
95 |                 self.push(value);
96 |                 Ok(())
97 |             }
98 |         }
99 |     }
100 | 
101 |     pub fn read_all<T>(&mut self, values: ValueIterator<'_, T>) -> Result<(), EvalError>
102 |     where
103 |         T: Heap,
104 |     {
105 |         for value in values {
106 |             self.read(value?)?;
107 |         }
108 |         Ok(())
109 |     }
110 | 
111 |     pub fn eval(&mut self) -> Result<Value, EvalError> {
112 |         while let Some(proc) = self.op_stack.pop() {
113 |             proc(&mut self.stack)?;
114 |         }
115 |         Ok(self.stack.pop().unwrap_or(Value::None))
116 |     }
117 | }
118 | 
119 | pub type NativeFn = fn(&mut Vec<Value>) -> Result<(), EvalError>;
120 | 
121 | #[cfg(test)]
122 | mod tests {
123 |     use super::*;
124 |     use crate::heap::Hash;
125 | 
126 |     struct NoHeap;
127 | 
128 |     impl Heap for NoHeap {
129 |         fn put(&mut self, _data: &[u8]) -> Hash {
130 |             unreachable!()
131 |         }
132 |     }
133 | 
134 |     #[test]
135 |     fn test_read_all_1() -> Result<(), EvalError> {
136 |         let input = "5";
137 |         let mut blobs = NoHeap;
138 |         let iter = ValueIterator::new(input, &mut blobs);
139 | 
140 |         let mut ctx = Context::new();
141 |         ctx.read_all(iter)?;
142 | 
143 |         assert!(ctx.stack.len() == 1);
144 |         assert_eq!(ctx.pop().unwrap().as_int(), Some(5));
145 |         Ok(())
146 |     }
147 | 
148 |     #[test]
149 |     fn test_eval_1() -> Result<(), EvalError> {
150 |         let input = "5";
151 |         let mut blobs = NoHeap;
152 |         let iter = ValueIterator::new(input, &mut blobs);
153 | 
154 |         let mut ctx = Context::new();
155 |         ctx.read_all(iter)?;
156 |         let result = ctx.eval()?;
157 | 
158 |         assert_eq!(result.as_int(), Some(5));
159 |         Ok(())
160 |     }
161 | 
162 |     #[test]
163 |     fn test_proc_1() -> Result<(), EvalError> {
164 |         let mut ctx = Context::new();
165 |         ctx.load_module(&crate::core::CORE_MODULE);
166 | 
167 |         let input = "add 7 8";
168 |         let mut blobs = NoHeap;
169 |         let iter = ValueIterator::new(input, &mut blobs);
170 | 
171 |         ctx.read_all(iter)?;
172 | 
173 |         let result = ctx.eval()?;
174 |         assert_eq!(result.as_int(), Some(15));
175 | 
176 |         Ok(())
177 |     }
178 | }
```

crates/rebeldb/src/func.rs
```
1 | // In runtime.rs:
2 | 
3 | /// Type for host functions that can be called from WebAssembly
4 | pub type HostFuncResult = Result<Vec<WasmValue>>;
5 | 
6 | /// Context passed to host functions
7 | #[derive(Default)]
8 | pub struct HostContext {
9 |     // Can be extended based on needs
10 |     pub memory: Option<Box<dyn WasmMemory>>,
11 |     // Add other context fields as needed
12 | }
13 | 
14 | /// Type for static host functions
15 | pub type StaticHostFn = fn(&HostContext, &[WasmValue]) -> HostFuncResult;
16 | 
17 | /// Configuration for a host function
18 | #[derive(Clone)]
19 | pub struct HostFuncConfig {
20 |     pub name: String,
21 |     pub params: Vec<WasmValueType>,
22 |     pub results: Vec<WasmValueType>,
23 |     pub func: StaticHostFn,
24 | }
25 | 
26 | /// WebAssembly value types for function signatures
27 | #[derive(Debug, Clone, PartialEq)]
28 | pub enum WasmValueType {
29 |     I32,
30 |     I64,
31 |     F32,
32 |     F64,
33 | }
34 | 
35 | /// Extended RuntimeConfig to include host functions
36 | #[derive(Default)]
37 | pub struct RuntimeConfig {
38 |     pub memory_pages: Option<u32>,
39 |     pub enable_threads: bool,
40 |     pub enable_simd: bool,
41 |     pub host_functions: Vec<HostFuncConfig>,
42 | }
43 | 
44 | /// Example of a host function implementation
45 | pub fn example_host_function(ctx: &HostContext, params: &[WasmValue]) -> HostFuncResult {
46 |     // Access memory if needed
47 |     if let Some(memory) = &ctx.memory {
48 |         // Do something with memory
49 |     }
50 | 
51 |     // Process parameters and return results
52 |     Ok(vec![WasmValue::I32(42)])
53 | }
54 | 
55 | // Modified WasmtimeRuntime implementation
56 | pub struct WasmtimeRuntime {
57 |     store: Store<HostContext>,
58 |     engine: Engine,
59 |     host_functions: Vec<HostFuncConfig>,
60 | }
61 | 
62 | impl WasmtimeRuntime {
63 |     pub fn new() -> Self {
64 |         let engine = Engine::default();
65 |         let store = Store::new(&engine, HostContext::default());
66 |         Self {
67 |             store,
68 |             engine,
69 |             host_functions: Vec::new(),
70 |         }
71 |     }
72 | 
73 |     fn register_host_function(&mut self, config: HostFuncConfig) -> Result<()> {
74 |         let func_type = wasmtime::FuncType::new(
75 |             config.params.iter().map(|t| match t {
76 |                 WasmValueType::I32 => wasmtime::ValType::I32,
77 |                 WasmValueType::I64 => wasmtime::ValType::I64,
78 |                 WasmValueType::F32 => wasmtime::ValType::F32,
79 |                 WasmValueType::F64 => wasmtime::ValType::F64,
80 |             }),
81 |             config.results.iter().map(|t| match t {
82 |                 WasmValueType::I32 => wasmtime::ValType::I32,
83 |                 WasmValueType::I64 => wasmtime::ValType::I64,
84 |                 WasmValueType::F32 => wasmtime::ValType::F32,
85 |                 WasmValueType::F64 => wasmtime::ValType::F64,
86 |             }),
87 |         );
88 | 
89 |         let host_func = config.func;
90 |         let func = wasmtime::Func::new(
91 |             &mut self.store,
92 |             func_type,
93 |             move |caller: wasmtime::Caller<'_, HostContext>,
94 |                   params: &[wasmtime::Val],
95 |                   results: &mut [wasmtime::Val]| {
96 |                 // Convert parameters
97 |                 let wasm_params: Vec<WasmValue> = params
98 |                     .iter()
99 |                     .map(|v| match v {
100 |                         wasmtime::Val::I32(x) => WasmValue::I32(*x),
101 |                         wasmtime::Val::I64(x) => WasmValue::I64(*x),
102 |                         wasmtime::Val::F32(x) => WasmValue::F32(f32::from_bits(*x)),
103 |                         wasmtime::Val::F64(x) => WasmValue::F64(f64::from_bits(*x)),
104 |                         _ => unreachable!(),
105 |                     })
106 |                     .collect();
107 | 
108 |                 // Call the host function with context
109 |                 let func_results = host_func(caller.data(), &wasm_params)
110 |                     .map_err(|e| wasmtime::Trap::new(format!("Host function error: {}", e)))?;
111 | 
112 |                 // Convert results
113 |                 for (i, result) in func_results.iter().enumerate() {
114 |                     results[i] = match result {
115 |                         WasmValue::I32(x) => wasmtime::Val::I32(*x),
116 |                         WasmValue::I64(x) => wasmtime::Val::I64(*x),
117 |                         WasmValue::F32(x) => wasmtime::Val::F32(x.to_bits()),
118 |                         WasmValue::F64(x) => wasmtime::Val::F64(x.to_bits()),
119 |                     };
120 |                 }
121 | 
122 |                 Ok(())
123 |             },
124 |         );
125 | 
126 |         self.host_functions.push(config);
127 |         Ok(())
128 |     }
129 | }
```

crates/rebeldb/src/heap.rs
```
1 | // RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // blob.rs:
4 | 
5 | use std::collections::HashMap;
6 | 
7 | pub type Hash = [u8; 32];
8 | 
9 | pub trait Heap {
10 |     fn put(&mut self, data: &[u8]) -> Hash;
11 | }
12 | 
13 | pub struct TempHeap {
14 |     data: HashMap<Hash, Vec<u8>>,
15 | }
16 | 
17 | impl Default for TempHeap {
18 |     fn default() -> Self {
19 |         Self::new()
20 |     }
21 | }
22 | 
23 | impl TempHeap {
24 |     pub fn new() -> Self {
25 |         Self {
26 |             data: HashMap::new(),
27 |         }
28 |     }
29 | }
30 | 
31 | impl Heap for TempHeap {
32 |     fn put(&mut self, data: &[u8]) -> Hash {
33 |         let hash = *blake3::hash(data).as_bytes();
34 |         self.data.insert(hash, data.to_vec());
35 |         hash
36 |     }
37 | }
```

crates/rebeldb/src/host.rs
```
1 | // RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // module.rs:
4 | 
5 | use crate::eval::NativeFn;
6 | use linkme::distributed_slice;
7 | 
8 | pub struct Module {
9 |     pub name: &'static str,
10 |     pub functions: &'static [(&'static str, NativeFn)],
11 | }
12 | 
13 | #[distributed_slice]
14 | pub static MODULES: [Module];
15 | 
16 | //
```

crates/rebeldb/src/lib.rs
```
1 | // RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // lib.rs:
4 | 
5 | pub mod boxed;
6 | pub mod codegen;
7 | pub mod core;
8 | pub mod eval;
9 | pub mod heap;
10 | pub mod parser;
11 | pub mod runtime;
12 | pub mod value;
13 | pub mod zerotime;
```

crates/rebeldb/src/parser.rs
```
1 | // RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // parser.rs:
4 | 
5 | use crate::heap::Heap;
6 | use crate::value::Value;
7 | use std::str::CharIndices;
8 | use thiserror::Error;
9 | 
10 | #[derive(Debug, Error)]
11 | pub enum ParseError {
12 |     #[error("Unexpected character: {0}")]
13 |     UnexpectedChar(char),
14 |     #[error("Unexpected end of input")]
15 |     UnexpectedEnd,
16 |     #[error("Number too large")]
17 |     NumberTooLarge,
18 |     #[error(transparent)]
19 |     ValueError(#[from] crate::value::ValueError),
20 | }
21 | 
22 | struct Token {
23 |     value: Value,
24 |     last_in_block: bool,
25 | }
26 | 
27 | impl Token {
28 |     fn new(value: Value, last_in_block: bool) -> Self {
29 |         Self {
30 |             value,
31 |             last_in_block,
32 |         }
33 |     }
34 | }
35 | 
36 | pub struct ValueIterator<'a, T>
37 | where
38 |     T: Heap,
39 | {
40 |     input: &'a str,
41 |     cursor: CharIndices<'a>,
42 |     blobs: &'a mut T,
43 | }
44 | 
45 | impl<T> Iterator for ValueIterator<'_, T>
46 | where
47 |     T: Heap,
48 | {
49 |     type Item = Result<Value, ParseError>;
50 | 
51 |     fn next(&mut self) -> Option<Self::Item> {
52 |         self.parse_value()
53 |             .map(|result| result.map(|token| token.value))
54 |     }
55 | }
56 | 
57 | impl<'a, T> ValueIterator<'a, T>
58 | where
59 |     T: Heap,
60 | {
61 |     pub fn new(input: &'a str, blobs: &'a mut T) -> Self {
62 |         Self {
63 |             cursor: input.char_indices(),
64 |             input,
65 |             blobs,
66 |         }
67 |     }
68 | 
69 |     fn skip_whitespace(&mut self) -> Option<(usize, char)> {
70 |         for (pos, char) in self.cursor.by_ref() {
71 |             if !char.is_ascii_whitespace() {
72 |                 return Some((pos, char));
73 |             }
74 |         }
75 |         None
76 |     }
77 | 
78 |     fn parse_string(&mut self, pos: usize) -> Result<Token, ParseError> {
79 |         let start_pos = pos + 1; // skip the opening quote
80 |         for (pos, char) in self.cursor.by_ref() {
81 |             if char == '"' {
82 |                 return Ok(Token::new(
83 |                     Value::string(&self.input[start_pos..pos], self.blobs),
84 |                     false,
85 |                 ));
86 |             }
87 |         }
88 | 
89 |         Err(ParseError::UnexpectedEnd)
90 |     }
91 | 
92 |     fn parse_word(&mut self, start_pos: usize) -> Result<Token, ParseError> {
93 |         for (pos, char) in self.cursor.by_ref() {
94 |             match char {
95 |                 c if c.is_ascii_alphanumeric() || c == '_' || c == '-' => {}
96 |                 ':' => {
97 |                     return Ok(Token::new(
98 |                         Value::set_word(&self.input[start_pos..pos])?,
99 |                         false,
100 |                     ))
101 |                 }
102 |                 c if c.is_ascii_whitespace() || c == ']' => {
103 |                     return Ok(Token::new(
104 |                         Value::word(&self.input[start_pos..pos])?,
105 |                         c == ']',
106 |                     ))
107 |                 }
108 |                 _ => return Err(ParseError::UnexpectedChar(char)),
109 |             }
110 |         }
111 |         Err(ParseError::UnexpectedEnd)
112 |     }
113 | 
114 |     fn parse_number(&mut self, char: char) -> Result<Token, ParseError> {
115 |         let mut value: i64 = 0;
116 |         let mut is_negative: Option<bool> = None;
117 |         let mut has_digits = false;
118 |         let mut end_of_block = false;
119 | 
120 |         match char {
121 |             '+' => {
122 |                 is_negative = Some(false);
123 |             }
124 |             '-' => {
125 |                 is_negative = Some(true);
126 |             }
127 |             c if c.is_ascii_digit() => {
128 |                 value = c.to_digit(10).unwrap() as i64;
129 |                 has_digits = true;
130 |             }
131 |             _ => return Err(ParseError::UnexpectedChar(char)),
132 |         }
133 | 
134 |         for (_, char) in self.cursor.by_ref() {
135 |             match char {
136 |                 c if c.is_ascii_digit() => {
137 |                     has_digits = true;
138 |                     value = value
139 |                         .checked_mul(10)
140 |                         .and_then(|v| v.checked_add(c.to_digit(10).unwrap() as i64))
141 |                         .ok_or(ParseError::NumberTooLarge)?;
142 |                 }
143 |                 ']' => {
144 |                     end_of_block = true;
145 |                     break;
146 |                 }
147 |                 _ => break,
148 |             }
149 |         }
150 | 
151 |         if !has_digits {
152 |             return Err(ParseError::UnexpectedEnd);
153 |         }
154 | 
155 |         match is_negative {
156 |             Some(true) => Ok(Token::new(Value::Int(-value), end_of_block)),
157 |             _ => Ok(Token::new(Value::Int(value), end_of_block)),
158 |         }
159 |     }
160 | 
161 |     fn parse_value(&mut self) -> Option<Result<Token, ParseError>> {
162 |         match self.skip_whitespace() {
163 |             None => None,
164 |             Some((pos, char)) => match char {
165 |                 '[' => self.parse_block(),
166 |                 '"' => Some(self.parse_string(pos)),
167 |                 c if c.is_ascii_alphabetic() => Some(self.parse_word(pos)),
168 |                 c if c.is_ascii_digit() || c == '+' || c == '-' => Some(self.parse_number(c)),
169 |                 _ => Some(Err(ParseError::UnexpectedChar(char))),
170 |             },
171 |         }
172 |     }
173 | 
174 |     fn parse_block(&mut self) -> Option<Result<Token, ParseError>> {
175 |         let mut values = Vec::<Value>::new();
176 |         loop {
177 |             match self.parse_value() {
178 |                 Some(Ok(Token {
179 |                     value,
180 |                     last_in_block,
181 |                 })) => {
182 |                     values.push(value);
183 |                     if last_in_block {
184 |                         break;
185 |                     }
186 |                 }
187 |                 Some(Err(err)) => return Some(Err(err)),
188 |                 None => {
189 |                     if values.is_empty() {
190 |                         return None;
191 |                     } else {
192 |                         break;
193 |                     }
194 |                 }
195 |             }
196 |         }
197 | 
198 |         Some(
199 |             Value::block(&values, self.blobs)
200 |                 .map_err(ParseError::ValueError)
201 |                 .map(|v| Token::new(v, false)),
202 |         )
203 |     }
204 | }
205 | 
206 | #[cfg(test)]
207 | mod tests {
208 |     use super::*;
209 |     use crate::heap::Hash;
210 | 
211 |     struct NullStorage;
212 | 
213 |     impl Heap for NullStorage {
214 |         fn put(&mut self, _data: &[u8]) -> Hash {
215 |             unreachable!()
216 |         }
217 |     }
218 | 
219 |     #[test]
220 |     fn test_whitespace_1() {
221 |         let input = "  \t\n  ";
222 |         let mut blobs = NullStorage;
223 |         let mut iter = ValueIterator::new(input, &mut blobs);
224 | 
225 |         let value = iter.next();
226 |         assert!(value.is_none());
227 |     }
228 | 
229 |     #[test]
230 |     fn test_string_1() -> anyhow::Result<()> {
231 |         let input = "\"hello\"  \n ";
232 |         let mut blobs = NullStorage;
233 |         let mut iter = ValueIterator::new(input, &mut blobs);
234 | 
235 |         let value = iter.next().unwrap().unwrap();
236 | 
237 |         unsafe {
238 |             assert_eq!(value.inlined_as_str(), Some("hello"));
239 |         }
240 | 
241 |         let value = iter.next();
242 |         assert!(value.is_none());
243 | 
244 |         Ok(())
245 |     }
246 | 
247 |     #[test]
248 |     fn test_number_1() -> anyhow::Result<()> {
249 |         let input = "42";
250 |         let mut blobs = NullStorage;
251 |         let mut iter = ValueIterator::new(input, &mut blobs);
252 | 
253 |         let value = iter.next().unwrap().unwrap();
254 |         assert_eq!(value.as_int(), Some(42));
255 | 
256 |         let value = iter.next();
257 |         assert!(value.is_none());
258 | 
259 |         Ok(())
260 |     }
261 | }
```

crates/rebeldb/src/runtime.rs
```
1 | // RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // runtime.rs:
4 | 
5 | //! WebAssembly Runtime Abstraction
6 | //!
7 | //! This crate provides a platform-agnostic abstraction layer for WebAssembly runtime operations.
8 | //! The goal is to provide a consistent interface for working with WebAssembly across different
9 | //! environments and runtime implementations.
10 | //!
11 | //! # Design Principles
12 | //!
13 | //! - **Runtime Agnostic**: The abstraction should work with any WebAssembly runtime (Wasmtime, Wasmer, etc.)
14 | //!   without being tied to specific runtime implementation details.
15 | //!
16 | //! - **Platform Independent**: While not all platforms may support all features, the core abstractions
17 | //!   should be usable across native, web, and mobile environments.
18 | //!
19 | //! - **Memory-First**: WebAssembly memory operations are fundamental and should work consistently
20 | //!   across all implementations, even when execution capabilities vary.
21 | //!
22 | //! - **Minimal Runtime Requirements**: Implementations can provide different levels of functionality,
23 | //!   from full execution environments to minimal memory-only implementations.
24 | //!
25 | //! # Runtime Implementations
26 | //!
27 | //! ## Wasmtime Runtime
28 | //!
29 | //! A full-featured implementation using Wasmtime as the backend. Provides complete WebAssembly
30 | //! execution capabilities including host functions, memory operations, and module instantiation.
31 | //!
32 | //! ## Zerotime Runtime
33 | //!
34 | //! A special "null" implementation that provides memory management without execution capabilities.
35 | //! This implementation is useful for:
36 | //! - Testing and development without full runtime overhead
37 | //! - Memory preparation and manipulation separate from execution
38 | //! - Scenarios where only memory operations are needed
39 | //! - As a reference implementation showing minimal requirements for a runtime
40 | //!
41 | //! # Use Cases
42 | //!
43 | //! This abstraction is particularly useful for:
44 | //!
45 | //! - **Cross-Platform Applications**: Write WebAssembly interaction code once and run it anywhere
46 | //! - **Testing and Development**: Use lighter implementations like zerotime for testing
47 | //! - **Memory Management**: Handle WebAssembly memory consistently across different platforms
48 | //! - **Runtime Switching**: Easily swap between different WebAssembly runtimes based on needs
49 | //!
50 | //! # Memory Model
51 | //!
52 | //! All implementations share the same WebAssembly memory model:
53 | //! - Memory is organized in 64KB pages
54 | //! - Linear memory is represented as contiguous bytes
55 | //! - Standard operations: read, write, grow
56 | //! - Memory can be shared between different runtime implementations
57 | //!
58 | //! # Example
59 | //!
60 | //! ```rust,no_run
61 | //! use rebeldb::runtime::{WasmRuntime, RuntimeConfig};
62 | //!
63 | //! // This code will work with any runtime implementation
64 | //! fn process_wasm<R: WasmRuntime>(runtime: &mut R, wasm_bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
65 | //!     let mut instance = runtime.instantiate_module(wasm_bytes)?;
66 | //!     let memory = instance.get_memory("memory")?;
67 | //!     // ... work with memory
68 | //!     Ok(())
69 | //! }
70 | //! ```
71 | 
72 | /// Common WebAssembly value types
73 | #[derive(Debug, Clone)]
74 | pub enum WasmValue {
75 |     I32(i32),
76 |     I64(i64),
77 |     F32(f32),
78 |     F64(f64),
79 | }
80 | 
81 | /// Common error types across all runtimes
82 | #[derive(Debug, thiserror::Error)]
83 | pub enum WasmError {
84 |     #[error("Failed to instantiate module: {0}")]
85 |     Instantiation(String),
86 |     #[error("Runtime error: {0}")]
87 |     Runtime(String),
88 |     #[error("Memory error: {0}")]
89 |     Memory(String),
90 |     #[error("Function not found: {0}")]
91 |     FunctionNotFound(String),
92 | }
93 | 
94 | pub type Result<T> = std::result::Result<T, WasmError>;
95 | 
96 | /// Abstract memory interface
97 | pub trait WasmMemory {
98 |     fn size(&self) -> usize;
99 |     fn grow(&mut self, pages: u32) -> Result<()>;
100 |     fn read(&self, offset: usize, buf: &mut [u8]) -> Result<()>;
101 |     fn write(&mut self, offset: usize, data: &[u8]) -> Result<()>;
102 | }
103 | 
104 | /// Abstract instance interface
105 | pub trait WasmInstance {
106 |     fn get_memory(&mut self, name: &str) -> Result<Box<dyn WasmMemory + '_>>;
107 |     fn call_function(&mut self, name: &str, params: &[WasmValue]) -> Result<Vec<WasmValue>>;
108 | }
109 | 
110 | /// The main runtime trait that all platforms will implement
111 | pub trait WasmRuntime {
112 |     fn instantiate_module(&mut self, wasm_bytes: &[u8]) -> Result<Box<dyn WasmInstance>>;
113 | 
114 |     // Optional method for runtime-specific configurations
115 |     fn with_config(config: RuntimeConfig) -> Result<Self>
116 |     where
117 |         Self: Sized;
118 | }
119 | 
120 | /// Configuration options for runtime initialization
121 | #[derive(Debug, Clone, Default)]
122 | pub struct RuntimeConfig {
123 |     pub memory_pages: Option<u32>,
124 |     pub enable_threads: bool,
125 |     pub enable_simd: bool,
126 | }
```

crates/rebeldb/src/value.rs
```
1 | // RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // value.rs:
4 | 
5 | use crate::heap::Heap;
6 | use std::fmt;
7 | use std::io::{self, Write};
8 | use thiserror::Error;
9 | 
10 | #[derive(Debug, Error)]
11 | pub enum ValueError {
12 |     #[error(transparent)]
13 |     Utf8Error(#[from] std::str::Utf8Error),
14 |     #[error(transparent)]
15 |     IOError(#[from] io::Error),
16 |     #[error("index out of bounds {0} [0..{1}]")]
17 |     OutOfBounds(usize, usize),
18 | }
19 | 
20 | pub type Result<T> = std::result::Result<T, ValueError>;
21 | 
22 | pub trait Serialize {
23 |     fn serialize<W: Write>(&self, writer: &mut W) -> Result<usize>;
24 | }
25 | 
26 | pub trait Deserialize: Sized {
27 |     fn deserialize(bytes: &[u8]) -> Result<Self>;
28 | }
29 | 
30 | // should we stick to 32 + 4 - 2 bytes for better support of 32-bit systems?
31 | const INLINE_CONTENT_BUFFER: usize = 32 + 8 - 2;
32 | 
33 | pub const CONTENT_TYPE_UNKNOWN: u8 = 0x00;
34 | const CONTENT_TYPE_UTF8: u8 = 0x01;
35 | 
36 | pub enum ValueType {
37 |     None,
38 |     Int,
39 |     Float,
40 |     Bytes,
41 |     Hash,
42 |     PubKey,
43 |     Word,
44 |     SetWord,
45 |     Block,
46 |     Context,
47 | }
48 | 
49 | #[derive(Clone)]
50 | pub enum Value {
51 |     None,
52 | 
53 |     // Following types directly map to Wasm value types
54 |     Int(i64),
55 |     Float(f64),
56 | 
57 |     Bytes(u8, Content),
58 | 
59 |     Hash([u8; 32]),
60 |     PubKey([u8; 32]),
61 | 
62 |     Word(Symbol),
63 |     SetWord(Symbol),
64 | 
65 |     Block(Content),
66 |     Context(Content),
67 | 
68 |     NativeFn(usize, usize),
69 | }
70 | 
71 | impl Value {
72 |     pub fn none() -> Self {
73 |         Self::None
74 |     }
75 | 
76 |     pub fn string(str: &str, heap: &mut impl Heap) -> Self {
77 |         Self::Bytes(CONTENT_TYPE_UTF8, Content::new(str.as_bytes(), heap))
78 |     }
79 | 
80 |     pub fn word(str: &str) -> Result<Self> {
81 |         Ok(Self::Word(Symbol::new(str)?))
82 |     }
83 | 
84 |     pub fn set_word(str: &str) -> Result<Self> {
85 |         Ok(Self::SetWord(Symbol::new(str)?))
86 |     }
87 | 
88 |     pub fn block(block: &[Value], heap: &mut impl Heap) -> Result<Self> {
89 |         let mut bytes = Vec::new();
90 |         for value in block {
91 |             value.serialize(&mut bytes)?;
92 |         }
93 |         Ok(Self::Block(Content::new(&bytes, heap)))
94 |     }
95 | 
96 |     pub fn context(context: &[(Symbol, Value)], heap: &mut impl Heap) -> Result<Self> {
97 |         let mut bytes = Vec::new();
98 |         for value in context {
99 |             value.0.serialize(&mut bytes)?;
100 |             value.1.serialize(&mut bytes)?;
101 |         }
102 |         Ok(Self::Context(Content::new(&bytes, heap)))
103 |     }
104 | 
105 |     pub fn as_int(&self) -> Option<i64> {
106 |         match self {
107 |             Value::Int(x) => Some(*x),
108 |             _ => None,
109 |         }
110 |     }
111 | 
112 |     /// Attempts to extract a string representation of the value
113 |     ///
114 |     /// # Returns
115 |     /// - `Some(&str)`: Always returns symbol for all word variants
116 |     /// - `Some(&str)`: For `Bytes` which are `Utf8` encoded if content fits in the inline buffer
117 |     /// - `None`: For all other variants
118 |     ///
119 |     /// # Safety
120 |     /// This method is safe because:
121 |     /// - The serialization format preserves `Utf8` encoding
122 |     /// - The data is immutable after deserialization
123 |     pub unsafe fn inlined_as_str(&self) -> Option<&str> {
124 |         match self {
125 |             Value::Bytes(CONTENT_TYPE_UTF8, content) => content
126 |                 .inlined()
127 |                 .map(|bytes| std::str::from_utf8_unchecked(bytes)),
128 |             Value::Word(symbol) => Some(symbol.symbol()),
129 |             Value::SetWord(symbol) => Some(symbol.symbol()),
130 |             _ => None,
131 |         }
132 |     }
133 | }
134 | 
135 | //
136 | 
137 | #[derive(Clone, Copy, Debug, PartialEq, Eq)]
138 | pub struct BoxedValue(u64);
139 | 
140 | /// We force the exponent bits (62..52) to 0x7FF, ensuring it's a NaN if fraction != 0.
141 | /// sign bit (63) is free for marking negative vs. positive integers.
142 | /// fraction (52 bits): top 4 bits (51..48) are the tag, lower 48 bits (47..0) are payload.
143 | const EXP_SHIFT: u64 = 52;
144 | const EXP_MAX: u64 = 0x7FF; // exponent bits all 1 => 0x7FF
145 | const EXP_MASK: u64 = EXP_MAX << EXP_SHIFT; // bits 62..52
146 | 
147 | /// Bit positions for tag and sign
148 | const TAG_SHIFT: u64 = 48; // so bits 51..48 are the tag
149 | const TAG_MASK: u64 = 0xF; // 4 bits
150 | 
151 | /// In this layout:
152 | ///  bit 63 = sign
153 | ///  bits 62..52 = exponent = 0x7FF
154 | ///  bits 51..48 = tag
155 | ///  bits 47..0  = payload
156 | 
157 | /// Example tags:
158 | #[repr(u64)]
159 | #[derive(Clone, Copy, Debug, PartialEq, Eq)]
160 | enum Tag {
161 |     Int = 0x0, // up to you which nibble you choose
162 |     Ptr = 0x1,
163 |     // You can define up to 16 different tags (0x0 .. 0xF).
164 | }
165 | 
166 | impl BoxedValue {
167 |     /// Create a boxed *signed* integer.
168 |     /// Uses the top (bit 63) as the "sign bit" for negative vs. non-negative.
169 |     /// The integer's absolute value must fit in 48 bits: -(2^47) .. 2^47 - 1.
170 |     pub fn new_int(value: i64) -> Self {
171 |         // sign bit: 1 if negative, 0 otherwise
172 |         let sign_bit = if value < 0 { 1 } else { 0 };
173 |         let mag = value.unsigned_abs(); // absolute value as u64
174 | 
175 |         // Ensure fits in 48 bits
176 |         assert!(
177 |             mag < (1u64 << 48),
178 |             "Integer out of range for 48-bit magnitude"
179 |         );
180 | 
181 |         // fraction = [ tag(4 bits) | payload(48 bits) ]
182 |         // top 4 bits of fraction => Tag::Int
183 |         // lower 48 bits => magnitude
184 |         let fraction = ((Tag::Int as u64) & TAG_MASK) << TAG_SHIFT | (mag & 0xFFFF_FFFF_FFFF);
185 | 
186 |         // Combine sign, exponent=0x7FF, fraction
187 |         let bits = (sign_bit << 63) | EXP_MASK | fraction;
188 |         BoxedValue(bits)
189 |     }
190 | 
191 |     /// Try to interpret this BoxedValue as an integer.
192 |     pub fn as_int(&self) -> i64 {
193 |         let sign_bit = (self.0 >> 63) & 1;
194 |         // Check exponent == 0x7FF
195 |         let exponent = (self.0 >> EXP_SHIFT) & 0x7FF;
196 |         // Check tag
197 |         let tag = (self.0 >> TAG_SHIFT) & TAG_MASK;
198 |         // Check fraction != 0 => must be a NaN, not Inf
199 |         let fraction = self.0 & ((1u64 << TAG_SHIFT) - 1u64 | (TAG_MASK << TAG_SHIFT));
200 | 
201 |         // Validate that it *looks* like a NaN-boxed integer
202 |         assert_eq!(exponent, EXP_MAX, "Not a NaN exponent");
203 |         assert_ne!(fraction, 0, "Looks like Infinity, not NaN");
204 |         assert_eq!(tag, Tag::Int as u64, "Not an Int tag");
205 | 
206 |         // Lower 48 bits = magnitude
207 |         let mag = self.0 & 0x000F_FFFF_FFFF_FFFF; // mask out exponent & sign & top 4 bits
208 |         let magnitude_48 = mag & 0xFFFF_FFFF_FFFF; // bits 47..0
209 | 
210 |         if sign_bit == 0 {
211 |             // positive or zero
212 |             magnitude_48 as i64
213 |         } else {
214 |             // negative
215 |             -(magnitude_48 as i64)
216 |         }
217 |     }
218 | 
219 |     /// Create a boxed pointer (for 32-bit addresses).
220 |     /// Tag = Tag::Ptr, exponent=0x7FF, sign=0, fraction bits 47..0 store the pointer.
221 |     pub fn new_ptr(addr: u32) -> Self {
222 |         // If you need more than 32 bits, store additional bits as needed.
223 |         let fraction = ((Tag::Ptr as u64) & TAG_MASK) << TAG_SHIFT | (addr as u64);
224 |         let bits = (0 << 63) // sign = 0
225 |             | EXP_MASK
226 |             | fraction;
227 |         BoxedValue(bits)
228 |     }
229 | 
230 |     /// Try to interpret this BoxedValue as a 32-bit pointer.
231 |     pub fn as_ptr(&self) -> u32 {
232 |         let exponent = (self.0 >> EXP_SHIFT) & 0x7FF;
233 |         let tag = (self.0 >> TAG_SHIFT) & TAG_MASK;
234 |         let fraction = self.0 & 0x000F_FFFF_FFFF_FFFF;
235 | 
236 |         // Validate
237 |         assert_eq!(exponent, EXP_MAX, "Not a NaN exponent");
238 |         assert_ne!(fraction, 0, "Looks like Infinity, not NaN");
239 |         assert_eq!(tag, Tag::Ptr as u64, "Not a Ptr tag");
240 | 
241 |         // Just grab the lower 32 bits
242 |         (fraction & 0xFFFF_FFFF) as u32
243 |     }
244 | 
245 |     /// Returns the raw bits for debugging or advanced use
246 |     pub fn bits(&self) -> u64 {
247 |         self.0
248 |     }
249 | }
250 | 
251 | //
252 | 
253 | const TAG_NONE: u8 = 0x00;
254 | const TAG_INT: u8 = 0x01;
255 | const TAG_FLOAT: u8 = 0x02;
256 | const TAG_BYTES: u8 = 0x04;
257 | const TAG_WORD: u8 = 0x05;
258 | const TAG_SET_WORD: u8 = 0x06;
259 | const TAG_BLOCK: u8 = 0x07;
260 | 
261 | fn write_slices<W: Write>(writer: &mut W, slices: &[&[u8]]) -> Result<usize> {
262 |     let mut total_size = 0;
263 |     for slice in slices {
264 |         writer.write_all(slice)?;
265 |         total_size += slice.len();
266 |     }
267 |     Ok(total_size)
268 | }
269 | 
270 | fn write_tag<W: Write>(writer: &mut W, tag: u8) -> Result<usize> {
271 |     write_slices(writer, &[&[tag]])
272 | }
273 | 
274 | fn write_tag_slice<W: Write>(writer: &mut W, tag: u8, slice: &[u8]) -> Result<usize> {
275 |     write_slices(writer, &[&[tag], slice])
276 | }
277 | 
278 | fn write_word<W: Write>(writer: &mut W, tag: u8, symbol: &Symbol) -> Result<usize> {
279 |     let tag_size = write_tag(writer, tag)?;
280 |     let symbol_size = symbol.serialize(writer)?;
281 |     Ok(tag_size + symbol_size)
282 | }
283 | 
284 | impl Serialize for Value {
285 |     fn serialize<W: Write>(&self, writer: &mut W) -> Result<usize> {
286 |         match self {
287 |             Value::None => write_tag(writer, TAG_NONE),
288 |             Value::Int(x) => write_tag_slice(writer, TAG_INT, &x.to_le_bytes()),
289 |             Value::Float(x) => write_tag_slice(writer, TAG_FLOAT, &x.to_le_bytes()),
290 |             Value::Bytes(enc, content) => {
291 |                 writer.write_all(&[TAG_BYTES, *enc])?;
292 |                 let size = content.serialize(writer)?;
293 |                 Ok(size + 2)
294 |             }
295 |             Value::Word(x) => write_word(writer, TAG_WORD, x),
296 |             Value::SetWord(x) => write_word(writer, TAG_SET_WORD, x),
297 |             Value::Block(content) => {
298 |                 writer.write_all(&[TAG_BLOCK])?;
299 |                 let size = content.serialize(writer)?;
300 |                 Ok(size + 1)
301 |             }
302 |             _ => unimplemented!(),
303 |         }
304 |     }
305 | }
306 | 
307 | macro_rules! read_numeric_value {
308 |     ($bytes:expr, $type:ty, $constructor:expr) => {{
309 |         const LEN: usize = std::mem::size_of::<$type>() + 1;
310 |         if $bytes.len() < LEN {
311 |             return Err(ValueError::OutOfBounds(LEN, $bytes.len()));
312 |         }
313 |         let mut buf = [0u8; std::mem::size_of::<$type>()];
314 |         buf.copy_from_slice(&$bytes[1..LEN]);
315 |         Ok($constructor(<$type>::from_le_bytes(buf)))
316 |     }};
317 | }
318 | 
319 | macro_rules! read_word {
320 |     ($bytes:expr, $constructor:expr) => {{
321 |         let symbol = Symbol::deserialize(&$bytes[1..])?;
322 |         Ok($constructor(symbol))
323 |     }};
324 | }
325 | 
326 | impl Deserialize for Value {
327 |     fn deserialize(bytes: &[u8]) -> Result<Self> {
328 |         if bytes.is_empty() {
329 |             return Err(ValueError::OutOfBounds(0, 1));
330 |         }
331 |         let tag = bytes[0];
332 |         match tag {
333 |             TAG_NONE => Ok(Value::None),
334 |             TAG_INT => read_numeric_value!(bytes, i64, Value::Int),
335 |             TAG_FLOAT => read_numeric_value!(bytes, f64, Value::Float),
336 |             TAG_WORD => read_word!(bytes, Value::Word),
337 |             TAG_SET_WORD => read_word!(bytes, Value::SetWord),
338 |             TAG_BYTES => {
339 |                 if bytes.len() < 2 {
340 |                     return Err(ValueError::OutOfBounds(2, bytes.len()));
341 |                 }
342 |                 let enc = bytes[1];
343 |                 let content = Content::deserialize(&bytes[2..])?;
344 |                 Ok(Value::Bytes(enc, content))
345 |             }
346 |             TAG_BLOCK => {
347 |                 let content = Content::deserialize(&bytes[1..])?;
348 |                 Ok(Value::Block(content))
349 |             }
350 |             _ => unimplemented!(),
351 |         }
352 |     }
353 | }
354 | 
355 | impl fmt::Display for Value {
356 |     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
357 |         match self {
358 |             Value::None => write!(f, "None"),
359 |             Value::Int(x) => write!(f, "{}", x),
360 |             Value::Float(x) => write!(f, "{}", x),
361 |             Value::Bytes(CONTENT_TYPE_UTF8, _) => {
362 |                 write!(f, "{}", unsafe { self.inlined_as_str().unwrap() })
363 |             }
364 |             Value::Word(x) => write!(f, "{}", unsafe { x.symbol() }),
365 |             Value::SetWord(x) => write!(f, "{}:", unsafe { x.symbol() }),
366 |             Value::Block(_) => write!(f, "Block(...)"),
367 |             Value::NativeFn(module, proc) => {
368 |                 write!(f, "native proc: module {}, proc {}", module, proc)
369 |             }
370 |             _ => unimplemented!(),
371 |         }
372 |     }
373 | }
374 | 
375 | // C O N T E N T
376 | 
377 | #[derive(Clone)]
378 | pub struct Content {
379 |     content: [u8; INLINE_CONTENT_BUFFER],
380 | }
381 | 
382 | impl Content {
383 |     pub fn new(content: &[u8], heap: &mut impl Heap) -> Self {
384 |         let len = content.len();
385 |         if len < INLINE_CONTENT_BUFFER {
386 |             let mut buffer = [0u8; INLINE_CONTENT_BUFFER];
387 |             buffer[0] = len as u8;
388 |             buffer[1..len + 1].copy_from_slice(&content[..len]);
389 |             Self { content: buffer }
390 |         } else {
391 |             let hash = heap.put(content);
392 |             let mut buffer = [0u8; INLINE_CONTENT_BUFFER];
393 |             buffer[0] = 0xff;
394 |             buffer[1..33].copy_from_slice(&hash);
395 |             Self { content: buffer }
396 |         }
397 |     }
398 | 
399 |     fn inlined(&self) -> Option<&[u8]> {
400 |         let len = self.content[0] as usize;
401 |         if len < INLINE_CONTENT_BUFFER {
402 |             Some(&self.content[1..len + 1])
403 |         } else {
404 |             None
405 |         }
406 |     }
407 | }
408 | 
409 | impl Serialize for Content {
410 |     fn serialize<W: Write>(&self, writer: &mut W) -> Result<usize> {
411 |         let len = self.content[0] as usize;
412 |         let len = if len < INLINE_CONTENT_BUFFER { len } else { 32 };
413 |         writer.write_all(&self.content[0..len + 1])?;
414 |         Ok(len + 1)
415 |     }
416 | }
417 | 
418 | impl Deserialize for Content {
419 |     fn deserialize(bytes: &[u8]) -> Result<Self> {
420 |         let mut content = [0u8; INLINE_CONTENT_BUFFER];
421 |         let len = bytes
422 |             .first()
423 |             .copied()
424 |             .ok_or(ValueError::OutOfBounds(0, 1))? as usize;
425 |         let len = if len < INLINE_CONTENT_BUFFER { len } else { 32 };
426 |         if bytes.len() < len + 1 {
427 |             return Err(ValueError::OutOfBounds(len + 1, bytes.len()));
428 |         }
429 |         content[..len + 1].copy_from_slice(&bytes[..len + 1]);
430 |         Ok(Self { content })
431 |     }
432 | }
433 | 
434 | impl std::fmt::Debug for Content {
435 |     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
436 |         // dump content in hexdump format with ascii representation
437 |         // Hexdump-like format, 16 bytes per line
438 |         for chunk in self.content.to_vec().chunks(16) {
439 |             write!(f, "      ")?;
440 |             for b in chunk {
441 |                 write!(f, "{:02x} ", b)?;
442 |             }
443 |             for _ in chunk.len()..16 {
444 |                 write!(f, "   ")?;
445 |             }
446 |             write!(f, " |")?;
447 |             for &b in chunk {
448 |                 let c = if b.is_ascii_graphic() || b == b' ' {
449 |                     b as char
450 |                 } else {
451 |                     '.'
452 |                 };
453 |                 write!(f, "{}", c)?;
454 |             }
455 |             for _ in chunk.len()..16 {
456 |                 write!(f, " ")?;
457 |             }
458 |             writeln!(f, "|")?;
459 |         }
460 |         writeln!(f)?;
461 |         Ok(())
462 |     }
463 | }
464 | 
465 | impl std::fmt::Debug for Value {
466 |     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
467 |         match self {
468 |             Value::None => write!(f, "None"),
469 |             Value::Int(x) => write!(f, "{}", x),
470 |             Value::Float(x) => write!(f, "{}", x),
471 |             Value::Bytes(enc, content) => {
472 |                 writeln!(f, "Bytes ({:02x})", enc)?;
473 |                 writeln!(f, "{:?}", content)
474 |             }
475 |             Value::Hash(hash) => write!(f, "Hash({})", hex::encode(hash)),
476 |             Value::PubKey(hash) => write!(f, "PubKey({})", hex::encode(hash)),
477 |             Value::Word(symbol) => write!(f, "Word({})", unsafe { symbol.symbol() }),
478 |             Value::SetWord(symbol) => write!(f, "SetWord({})", unsafe { symbol.symbol() }),
479 |             Value::Block(content) => write!(f, "Block({:?})", content),
480 |             Value::Context(content) => write!(f, "Context({:?})", content),
481 |             Value::NativeFn(module, proc) => {
482 |                 write!(f, "NativeFn(module: {}, proc: {})", module, proc)
483 |             }
484 |         }
485 |     }
486 | }
487 | 
488 | // S Y M B O L
489 | 
490 | #[derive(Debug, Clone, PartialEq, Eq, Hash)]
491 | pub struct Symbol {
492 |     symbol: [u8; INLINE_CONTENT_BUFFER],
493 | }
494 | 
495 | impl Symbol {
496 |     pub fn new(content: &str) -> Result<Self> {
497 |         let len = content.len();
498 |         if len < INLINE_CONTENT_BUFFER {
499 |             let mut symbol = [0u8; INLINE_CONTENT_BUFFER];
500 |             symbol[0] = len as u8;
501 |             symbol[1..len + 1].copy_from_slice(&content.as_bytes()[..len]);
502 |             Ok(Self { symbol })
503 |         } else {
504 |             Err(ValueError::OutOfBounds(len, INLINE_CONTENT_BUFFER - 1))
505 |         }
506 |     }
507 | 
508 |     unsafe fn symbol(&self) -> &str {
509 |         let len = self.symbol[0] as usize;
510 |         std::str::from_utf8_unchecked(&self.symbol[1..len + 1])
511 |     }
512 | }
513 | 
514 | impl Serialize for Symbol {
515 |     fn serialize<W: Write>(&self, writer: &mut W) -> Result<usize> {
516 |         let len = self.symbol[0] as usize;
517 |         writer.write_all(&self.symbol[0..len + 1])?;
518 |         Ok(len + 1)
519 |     }
520 | }
521 | 
522 | impl Deserialize for Symbol {
523 |     fn deserialize(bytes: &[u8]) -> Result<Self> {
524 |         let mut symbol = [0u8; INLINE_CONTENT_BUFFER];
525 |         let len = bytes
526 |             .first()
527 |             .copied()
528 |             .ok_or(ValueError::OutOfBounds(0, 1))? as usize;
529 |         if bytes.len() < len + 1 {
530 |             return Err(ValueError::OutOfBounds(len + 1, bytes.len()));
531 |         }
532 |         symbol[..len + 1].copy_from_slice(&bytes[..len + 1]);
533 |         Ok(Self { symbol })
534 |     }
535 | }
536 | 
537 | #[cfg(test)]
538 | mod tests {
539 |     use super::*;
540 | 
541 |     #[test]
542 |     fn test_content() {
543 |         let mut heap = crate::heap::TempHeap::new();
544 |         let content = Content::new(b"hello", &mut heap);
545 |         assert_eq!(content.content[0], 5);
546 |         assert_eq!(&content.content[1..6], b"hello");
547 |         let deserialized = Content::deserialize(&content.content).unwrap();
548 |         assert_eq!(content.content, deserialized.content);
549 |     }
550 | 
551 |     #[test]
552 |     fn test_symbol() {
553 |         let symbol = Symbol::new("hello").unwrap();
554 |         assert_eq!(symbol.symbol[0], 5);
555 |         assert_eq!(&symbol.symbol[1..6], b"hello");
556 |         let deserialized = Symbol::deserialize(&symbol.symbol).unwrap();
557 |         assert_eq!(symbol.symbol, deserialized.symbol);
558 |     }
559 | 
560 |     #[test]
561 |     fn test_context() -> Result<()> {
562 |         let mut heap = crate::heap::TempHeap::new();
563 |         let kv = vec![
564 |             (Symbol::new("hello")?, Value::Int(42)),
565 |             (Symbol::new("there")?, Value::Float(12341234.55)),
566 |             (Symbol::new("how")?, Value::Int(12341234)),
567 |             (Symbol::new("doing")?, Value::Float(1.12341234)),
568 |         ];
569 |         let ctx = Value::context(&kv, &mut heap)?;
570 |         match ctx {
571 |             Value::Context(content) => {
572 |                 println!("{:?}", content);
573 |             }
574 |             _ => panic!("expected Value::Context"),
575 |         }
576 |         Ok(())
577 |     }
578 | 
579 |     #[test]
580 |     fn test_value() -> Result<()> {
581 |         let mut heap = crate::heap::TempHeap::new();
582 |         let value = Value::string("hello", &mut heap);
583 |         let mut bytes = Vec::new();
584 |         value.serialize(&mut bytes)?;
585 |         let deserialized = Value::deserialize(&bytes).unwrap();
586 |         unsafe {
587 |             assert_eq!(deserialized.inlined_as_str(), Some("hello"));
588 |         }
589 |         Ok(())
590 |     }
591 | 
592 |     #[test]
593 |     fn test_string() -> Result<()> {
594 |         let mut heap = crate::heap::TempHeap::new();
595 |         let value = Value::string("hello, world!", &mut heap);
596 |         let mut bytes = Vec::new();
597 |         value.serialize(&mut bytes)?;
598 |         let deserialized = Value::deserialize(&bytes).unwrap();
599 |         unsafe {
600 |             assert_eq!(deserialized.inlined_as_str(), Some("hello, world!"));
601 |         }
602 |         println!("{:?}", deserialized);
603 |         Ok(())
604 |     }
605 | 
606 |     // B O X E D   V A L U E
607 | 
608 |     // #[test]
609 |     // fn test_int_round_trip() {
610 |     //     // Some sample values that fit in 48 bits
611 |     //     let cases = [
612 |     //         0,
613 |     //         42,
614 |     //         -42,
615 |     //         123_456_789,
616 |     //         -123_456_789,
617 |     //         (1 << 47) - 1,  // 140,737,488,355,327
618 |     //         -(1 << 47) + 1, // -140,737,488,355,327
619 |     //     ];
620 | 
621 |     //     for &val in &cases {
622 |     //         let boxed = BoxedValue::new_int(val);
623 |     //         let unboxed = boxed.as_int();
624 |     //         assert_eq!(
625 |     //             unboxed, val,
626 |     //             "Failed round-trip for {} => {:?} => {}",
627 |     //             val, boxed, unboxed
628 |     //         );
629 |     //     }
630 |     // }
631 | 
632 |     // #[test]
633 |     // #[should_panic(expected = "out of range")]
634 |     // fn test_int_overflow() {
635 |     //     // 2^47 is out of range
636 |     //     let _ = BoxedValue::new_int(1 << 47);
637 |     // }
638 | 
639 |     // #[test]
640 |     // fn test_ptr_round_trip() {
641 |     //     let ptrs = [0u32, 1, 0xDEAD_BEEF, 0xFFFF_FFFF];
642 | 
643 |     //     for &p in &ptrs {
644 |     //         let boxed = BoxedValue::new_ptr(p);
645 |     //         let unboxed = boxed.as_ptr();
646 |     //         assert_eq!(
647 |     //             unboxed, p,
648 |     //             "Failed round-trip for pointer {:08X} => {:?} => {:08X}",
649 |     //             p, boxed, unboxed
650 |     //         );
651 |     //     }
652 |     // }
653 | 
654 |     // #[test]
655 |     // fn test_bits_debug() {
656 |     //     let x = BoxedValue::new_int(42);
657 |     //     println!("Boxed bits for 42: 0x{:016X}", x.bits());
658 |     // }
659 | }
```

crates/rebeldb/src/zerotime.rs
```
1 | //
2 | 
3 | use crate::runtime::{
4 |     Result, RuntimeConfig, WasmError, WasmInstance, WasmMemory, WasmRuntime, WasmValue,
5 | };
6 | 
7 | // Our own memory implementation that matches WebAssembly memory model
8 | pub struct ZeroMemory {
9 |     data: Vec<u8>,
10 |     max_pages: Option<u32>,
11 | }
12 | 
13 | pub struct ZeroMemoryRef<'a> {
14 |     data: &'a mut Vec<u8>,
15 |     max_pages: Option<u32>,
16 | }
17 | 
18 | impl ZeroMemory {
19 |     const PAGE_SIZE: usize = 65536;
20 | 
21 |     fn new(initial_pages: u32, max_pages: Option<u32>) -> Self {
22 |         let size = initial_pages as usize * Self::PAGE_SIZE;
23 |         Self {
24 |             data: vec![0; size],
25 |             max_pages,
26 |         }
27 |     }
28 | }
29 | 
30 | pub struct ZeroInstance {
31 |     memory: ZeroMemory,
32 | }
33 | 
34 | pub struct ZeroRuntime {
35 |     // For now empty, might need configuration later
36 | }
37 | 
38 | impl<'a> WasmMemory for ZeroMemoryRef<'a> {
39 |     fn size(&self) -> usize {
40 |         self.data.len()
41 |     }
42 | 
43 |     fn grow(&mut self, additional_pages: u32) -> Result<()> {
44 |         let current_pages = self.data.len() / ZeroMemory::PAGE_SIZE;
45 |         let new_pages = current_pages + additional_pages as usize;
46 | 
47 |         if let Some(max) = self.max_pages {
48 |             if new_pages > max as usize {
49 |                 return Err(WasmError::Memory("Exceeded maximum memory pages".into()));
50 |             }
51 |         }
52 | 
53 |         let additional_bytes = additional_pages as usize * ZeroMemory::PAGE_SIZE;
54 |         self.data
55 |             .extend(std::iter::repeat(0).take(additional_bytes));
56 |         Ok(())
57 |     }
58 | 
59 |     fn read(&self, offset: usize, buf: &mut [u8]) -> Result<()> {
60 |         if offset + buf.len() > self.data.len() {
61 |             return Err(WasmError::Memory("Read outside memory bounds".into()));
62 |         }
63 |         buf.copy_from_slice(&self.data[offset..offset + buf.len()]);
64 |         Ok(())
65 |     }
66 | 
67 |     fn write(&mut self, offset: usize, data: &[u8]) -> Result<()> {
68 |         if offset + data.len() > self.data.len() {
69 |             return Err(WasmError::Memory("Write outside memory bounds".into()));
70 |         }
71 |         self.data[offset..offset + data.len()].copy_from_slice(data);
72 |         Ok(())
73 |     }
74 | }
75 | 
76 | impl WasmInstance for ZeroInstance {
77 |     fn get_memory(&mut self, _name: &str) -> Result<Box<dyn WasmMemory + '_>> {
78 |         Ok(Box::new(ZeroMemoryRef {
79 |             data: &mut self.memory.data,
80 |             max_pages: self.memory.max_pages,
81 |         }))
82 |     }
83 | 
84 |     fn call_function(&mut self, name: &str, _params: &[WasmValue]) -> Result<Vec<WasmValue>> {
85 |         Err(WasmError::Runtime(format!(
86 |             "Zerotime cannot execute functions (attempted to call {})",
87 |             name
88 |         )))
89 |     }
90 | }
91 | 
92 | impl WasmRuntime for ZeroRuntime {
93 |     fn instantiate_module(&mut self, _wasm_bytes: &[u8]) -> Result<Box<dyn WasmInstance>> {
94 |         // For now just create an instance with default memory
95 |         // Later we might want to parse the wasm binary to get memory specifications
96 |         Ok(Box::new(ZeroInstance {
97 |             memory: ZeroMemory::new(1, None), // Start with 1 page
98 |         }))
99 |     }
100 | 
101 |     fn with_config(_config: RuntimeConfig) -> Result<Self> {
102 |         Ok(Self {})
103 |     }
104 | }
```

crates/rebeldb-cli/src/main.rs
```
1 | // RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // main.rs:
4 | 
5 | use anyhow::Result;
6 | use colored::*;
7 | use rebeldb::eval::Context;
8 | use rebeldb::heap::TempHeap;
9 | use rebeldb::parser::ValueIterator;
10 | use rebeldb::value::Value;
11 | use rustyline::{error::ReadlineError, DefaultEditor};
12 | 
13 | fn evaluate(input: &str, heap: &mut TempHeap, ctx: &mut Context) -> Result<Value> {
14 |     ctx.read_all(ValueIterator::new(input, heap))?;
15 |     Ok(ctx.eval()?)
16 | }
17 | 
18 | fn main() -> Result<()> {
19 |     println!(
20 |         "{} © 2025 Huly Labs • {}",
21 |         "RebelDB™".bold(),
22 |         "https://hulylabs.com".underline()
23 |     );
24 |     println!("Type {} or press Ctrl+D to exit\n", ":quit".red().bold());
25 | 
26 |     // Initialize interpreter
27 |     //
28 |     let mut blobs = TempHeap::new();
29 |     let mut ctx = Context::new();
30 |     ctx.load_module(&rebeldb::core::CORE_MODULE);
31 | 
32 |     // Setup rustyline editor
33 |     let mut rl = DefaultEditor::new()?;
34 | 
35 |     // Load history from previous sessions
36 |     // let history_path = PathBuf::from(".history");
37 |     // if rl.load_history(&history_path).is_err() {
38 |     //     println!("No previous history.");
39 |     // }
40 | 
41 |     loop {
42 |         let readline = rl.readline(&"RebelDB™ ❯ ".to_string());
43 |         // let readline = rl.readline(&"RebelDB™ • ".to_string());
44 | 
45 |         match readline {
46 |             Ok(line) => {
47 |                 // Add to history
48 |                 rl.add_history_entry(line.as_str())?;
49 | 
50 |                 // Handle special commands
51 |                 if line.trim() == ":quit" {
52 |                     break;
53 |                 }
54 | 
55 |                 match evaluate(&line, &mut blobs, &mut ctx) {
56 |                     Ok(result) => println!("{}:  {}", "OK".green(), result),
57 |                     Err(err) => eprintln!("{}: {}", "ERR".red().bold(), err),
58 |                 }
59 |             }
60 |             Err(ReadlineError::Interrupted) => {
61 |                 println!("CTRL-C");
62 |                 continue;
63 |             }
64 |             Err(ReadlineError::Eof) => {
65 |                 println!("Bye!");
66 |                 break;
67 |             }
68 |             Err(err) => {
69 |                 println!("Error: {:?}", err);
70 |                 break;
71 |             }
72 |         }
73 |     }
74 | 
75 |     // Save history
76 |     // rl.save_history(&history_path)?;
77 | 
78 |     Ok(())
79 | }
```

crates/rebeldb-core/src/boxed.rs
```
1 | //
2 | 
3 | use thiserror::Error;
4 | 
5 | #[derive(Debug, Error)]
6 | pub enum BoxedError {
7 |     #[error("Integer out of 47-bit range")]
8 |     IntOutOfRange,
9 |     #[error("Not a NaN-boxed value")]
10 |     NotAQNan,
11 | }
12 | 
13 | #[derive(Clone, Copy, Debug, PartialEq, Eq)]
14 | pub struct BoxedValue(u64);
15 | 
16 | // We force exponent=0x7FF => bits 62..52
17 | const EXP_SHIFT: u64 = 52;
18 | const EXP_MAX: u64 = 0x7FF;
19 | const EXP_MASK: u64 = EXP_MAX << EXP_SHIFT; // bits 62..52 = all ones
20 | 
21 | // We'll always set fraction bit 51 = 1, so fraction != 0 => guaranteed NaN.
22 | const FRACTION_TOP_BIT: u64 = 1 << 51; // 0x8000_0000_0000
23 | 
24 | // 4-bit tag in bits 50..47
25 | const TAG_SHIFT: u64 = 47;
26 | const TAG_MASK: u64 = 0xF;
27 | 
28 | // That leaves bits 46..0 (47 bits) for the payload.
29 | const PAYLOAD_MASK_47: u64 = (1 << 47) - 1; // 0x7FFF_FFFF_FFFF
30 | 
31 | // To allow either sign bit (bit 63) to be 0 or 1, we mask off everything
32 | // except exponent (bits 62..52) and the top fraction bit (bit 51).
33 | // We compare against the pattern indicating exponent=0x7FF and fraction’s top bit=1.
34 | const QNAN_MASK: u64 = 0x7FF8_0000_0000_0000;
35 | 
36 | /// Example tags
37 | #[repr(u64)]
38 | #[derive(Clone, Copy, Debug, PartialEq, Eq)]
39 | enum Tag {
40 |     Int = 0x0,
41 |     WasmPtr = 0x1,
42 |     Float = 0x2,
43 |     Object = 0x3,
44 | }
45 | 
46 | impl BoxedValue {
47 |     /// Create a boxed *signed* integer with 47-bit 2's complement payload.
48 |     ///
49 |     /// Valid range: -2^46 .. 2^46 - 1
50 |     /// (i.e. about ±140.7 trillion)
51 |     pub fn new_int(value: i64) -> Self {
52 |         let payload_47 = ((value << (64 - 47)) >> (64 - 47)) as u64 & PAYLOAD_MASK_47;
53 |         let fraction = FRACTION_TOP_BIT | ((Tag::Int as u64) & TAG_MASK) << TAG_SHIFT | payload_47;
54 |         let bits = (0 << 63) | EXP_MASK | fraction;
55 |         BoxedValue(bits)
56 |     }
57 | 
58 |     /// Create a boxed *signed* integer with 47-bit 2's complement payload.
59 |     ///
60 |     /// Valid range: -2^46 .. 2^46 - 1
61 |     /// (i.e. about ±140.7 trillion)
62 |     pub fn safe_new_int(value: i64) -> Result<Self, BoxedError> {
63 |         let min = -(1 << 46); // -140,737,488,355,328
64 |         let max = (1 << 46) - 1; // +140,737,488,355,327
65 |         if value >= min && value <= max {
66 |             Ok(Self::new_int(value))
67 |         } else {
68 |             Err(BoxedError::IntOutOfRange)
69 |         }
70 |     }
71 | 
72 |     /// Interpret this BoxedValue as a 47-bit signed integer.
73 |     pub fn as_int(&self) -> i64 {
74 |         let bits = self.0;
75 |         let payload_47 = bits & PAYLOAD_MASK_47;
76 |         let shifted = (payload_47 << (64 - 47)) as i64; // cast to i64 => preserve bits
77 |         let value = shifted >> (64 - 47); // arithmetic shift right
78 |         value
79 |     }
80 | 
81 |     /// Interpret this BoxedValue as a 47-bit signed integer.
82 |     pub fn verify_nan(bits: u64) -> Result<(), BoxedError> {
83 |         if (bits & QNAN_MASK) == QNAN_MASK {
84 |             Ok(())
85 |         } else {
86 |             Err(BoxedError::NotAQNan)
87 |         }
88 |     }
89 | 
90 |     pub fn tag(&self) -> u8 {
91 |         let bits = self.0;
92 |         let fraction = bits & ((1 << 52) - 1); // lower 52 bits
93 |         ((fraction >> TAG_SHIFT) & TAG_MASK) as u8
94 |     }
95 | 
96 |     /// Create a boxed pointer (32 bits). Tag = Ptr, fraction bit 51=1, payload in bits 46..0.
97 |     pub fn new_ptr(addr: u32) -> Self {
98 |         let payload_47 = addr as u64; // zero-extended into 64
99 |                                       // We could store a 46- or 47-bit pointer, but typically 32 bits is enough.
100 | 
101 |         let fraction = FRACTION_TOP_BIT
102 |             | ((Tag::WasmPtr as u64) & TAG_MASK) << TAG_SHIFT
103 |             | (payload_47 & PAYLOAD_MASK_47);
104 | 
105 |         let bits = (0 << 63) | EXP_MASK | fraction;
106 |         BoxedValue(bits)
107 |     }
108 | 
109 |     /// Return the pointer as 32 bits.
110 |     pub fn as_ptr(&self) -> u32 {
111 |         let bits = self.0;
112 |         let payload_47 = bits & PAYLOAD_MASK_47;
113 |         payload_47 as u32
114 |     }
115 | 
116 |     /// Raw bits for debugging or advanced usage
117 |     pub fn bits(&self) -> u64 {
118 |         self.0
119 |     }
120 | }
121 | 
122 | #[inline(never)]
123 | pub fn tag(b: BoxedValue) -> u8 {
124 |     b.tag()
125 | }
126 | 
127 | #[inline(never)]
128 | pub fn verify(value: u64) -> Result<(), BoxedError> {
129 |     BoxedValue::verify_nan(value)
130 | }
131 | 
132 | #[inline(never)]
133 | pub fn box_int(value: i64) -> BoxedValue {
134 |     BoxedValue::new_int(value)
135 | }
136 | 
137 | #[inline(never)]
138 | pub fn safe_box_int(value: i64) -> Result<BoxedValue, BoxedError> {
139 |     BoxedValue::safe_new_int(value)
140 | }
141 | 
142 | #[inline(never)]
143 | pub fn unbox_int(b: BoxedValue) -> i64 {
144 |     b.as_int()
145 | }
146 | 
147 | #[inline(never)]
148 | pub fn box_ptr(addr: u32) -> BoxedValue {
149 |     BoxedValue::new_ptr(addr)
150 | }
151 | 
152 | #[inline(never)]
153 | pub fn unbox_ptr(b: BoxedValue) -> u32 {
154 |     b.as_ptr()
155 | }
156 | 
157 | #[cfg(test)]
158 | mod tests {
159 |     use super::*;
160 | 
161 |     #[test]
162 |     fn test_int_round_trip() {
163 |         let vals = [
164 |             0,
165 |             1,
166 |             -1,
167 |             42,
168 |             -42,
169 |             123_456_789,
170 |             -123_456_789,
171 |             (1 << 46) - 1, //  140,737,488,355,327
172 |             -(1 << 46),    // -140,737,488,355,328
173 |         ];
174 | 
175 |         for &v in &vals {
176 |             let b = BoxedValue::new_int(v);
177 |             let back = b.as_int();
178 |             assert_eq!(
179 |                 v,
180 |                 back,
181 |                 "Failed round-trip for {} => bits=0x{:016X} => {}",
182 |                 v,
183 |                 b.bits(),
184 |                 back
185 |             );
186 |         }
187 |     }
188 | 
189 |     #[test]
190 |     #[should_panic]
191 |     #[allow(arithmetic_overflow)]
192 |     fn test_int_out_of_range() {
193 |         // +2^46 is out of range: 140,737,488,355,328
194 |         BoxedValue::new_int((1 << 46) as i64);
195 |     }
196 | 
197 |     #[test]
198 |     fn test_ptr_round_trip() {
199 |         let ptrs = [0u32, 1, 0xDEAD_BEEF, 0xFFFF_FFFF];
200 |         for &p in &ptrs {
201 |             let b = BoxedValue::new_ptr(p);
202 |             let back = b.as_ptr();
203 |             assert_eq!(
204 |                 p,
205 |                 back,
206 |                 "Failed round-trip for pointer 0x{:08X} => bits=0x{:016X} => 0x{:08X}",
207 |                 p,
208 |                 b.bits(),
209 |                 back
210 |             );
211 |         }
212 |     }
213 | }
```

crates/rebeldb-core/src/lib.rs
```
1 | //
2 | 
3 | pub mod boxed;
```

crates/rebeldb-runtime/src/lib.rs
```
1 | // RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // lib.rs:
4 | 
5 | pub mod env;
```

crates/rebeldb-wasm/src/ctx.rs
```
1 | // RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // reboldb_wasm::ctx
4 | 
5 | use wasmtime::*;
6 | 
7 | struct MyState {
8 |     name: String,
9 |     count: usize,
10 | }
11 | 
12 | fn test1() -> Result<()> {
13 |     // First the wasm module needs to be compiled. This is done with a global
14 |     // "compilation environment" within an `Engine`. Note that engines can be
15 |     // further configured through `Config` if desired instead of using the
16 |     // default like this is here.
17 |     println!("Compiling module...");
18 |     let engine = Engine::default();
19 |     let module = Module::from_file(&engine, "examples/hello.wat")?;
20 | 
21 |     // After a module is compiled we create a `Store` which will contain
22 |     // instantiated modules and other items like host functions. A Store
23 |     // contains an arbitrary piece of host information, and we use `MyState`
24 |     // here.
25 |     println!("Initializing...");
26 |     let mut store = Store::new(
27 |         &engine,
28 |         MyState {
29 |             name: "hello, world!".to_string(),
30 |             count: 0,
31 |         },
32 |     );
33 | 
34 |     // Our wasm module we'll be instantiating requires one imported function.
35 |     // the function takes no parameters and returns no results. We create a host
36 |     // implementation of that function here, and the `caller` parameter here is
37 |     // used to get access to our original `MyState` value.
38 |     println!("Creating callback...");
39 |     let hello_func = Func::wrap(&mut store, |mut caller: Caller<'_, MyState>| {
40 |         println!("Calling back...");
41 |         println!("> {}", caller.data().name);
42 |         caller.data_mut().count += 1;
43 |     });
44 | 
45 |     // Once we've got that all set up we can then move to the instantiation
46 |     // phase, pairing together a compiled module as well as a set of imports.
47 |     // Note that this is where the wasm `start` function, if any, would run.
48 |     println!("Instantiating module...");
49 |     let imports = [hello_func.into()];
50 |     let instance = Instance::new(&mut store, &module, &imports)?;
51 | 
52 |     // Next we poke around a bit to extract the `run` function from the module.
53 |     println!("Extracting export...");
54 |     let run = instance.get_typed_func::<(), ()>(&mut store, "run")?;
55 | 
56 |     // And last but not least we can call it!
57 |     println!("Calling export...");
58 |     run.call(&mut store, ())?;
59 | 
60 |     println!("Done.");
61 |     Ok(())
62 | }
63 | 
64 | fn test2() -> Result<()> {
65 |     // Create our `store_fn` context and then compile a module and create an
66 |     // instance from the compiled module all in one go.
67 |     let mut store: Store<()> = Store::default();
68 |     let module = Module::from_file(store.engine(), "examples/memory.wat")?;
69 |     let instance = Instance::new(&mut store, &module, &[])?;
70 | 
71 |     // load_fn up our exports from the instance
72 |     let memory = instance
73 |         .get_memory(&mut store, "memory")
74 |         .ok_or(anyhow::format_err!("failed to find `memory` export"))?;
75 |     let size = instance.get_typed_func::<(), i32>(&mut store, "size")?;
76 |     let load_fn = instance.get_typed_func::<i32, i32>(&mut store, "load")?;
77 |     let store_fn = instance.get_typed_func::<(i32, i32), ()>(&mut store, "store")?;
78 | 
79 |     println!("Checking memory...");
80 |     assert_eq!(memory.size(&store), 2);
81 |     assert_eq!(memory.data_size(&store), 0x20000);
82 |     assert_eq!(memory.data_mut(&mut store)[0], 0);
83 |     assert_eq!(memory.data_mut(&mut store)[0x1000], 1);
84 |     assert_eq!(memory.data_mut(&mut store)[0x1003], 4);
85 | 
86 |     assert_eq!(size.call(&mut store, ())?, 2);
87 |     assert_eq!(load_fn.call(&mut store, 0)?, 0);
88 |     assert_eq!(load_fn.call(&mut store, 0x1000)?, 1);
89 |     assert_eq!(load_fn.call(&mut store, 0x1003)?, 4);
90 |     assert_eq!(load_fn.call(&mut store, 0x1ffff)?, 0);
91 |     assert!(load_fn.call(&mut store, 0x20000).is_err()); // out of bounds trap
92 | 
93 |     println!("Mutating memory...");
94 |     memory.data_mut(&mut store)[0x1003] = 5;
95 | 
96 |     store_fn.call(&mut store, (0x1002, 6))?;
97 |     assert!(store_fn.call(&mut store, (0x20000, 0)).is_err()); // out of bounds trap
98 | 
99 |     assert_eq!(memory.data(&store)[0x1002], 6);
100 |     assert_eq!(memory.data(&store)[0x1003], 5);
101 |     assert_eq!(load_fn.call(&mut store, 0x1002)?, 6);
102 |     assert_eq!(load_fn.call(&mut store, 0x1003)?, 5);
103 | 
104 |     // Grow memory.
105 |     println!("Growing memory...");
106 |     memory.grow(&mut store, 1)?;
107 |     assert_eq!(memory.size(&store), 3);
108 |     assert_eq!(memory.data_size(&store), 0x30000);
109 | 
110 |     assert_eq!(load_fn.call(&mut store, 0x20000)?, 0);
111 |     store_fn.call(&mut store, (0x20000, 0))?;
112 |     assert!(load_fn.call(&mut store, 0x30000).is_err());
113 |     assert!(store_fn.call(&mut store, (0x30000, 0)).is_err());
114 | 
115 |     assert!(memory.grow(&mut store, 1).is_err());
116 |     assert!(memory.grow(&mut store, 0).is_ok());
117 | 
118 |     println!("Creating stand-alone memory...");
119 |     let memorytype = MemoryType::new(5, Some(5));
120 |     let memory2 = Memory::new(&mut store, memorytype)?;
121 |     assert_eq!(memory2.size(&store), 5);
122 |     assert!(memory2.grow(&mut store, 1).is_err());
123 |     assert!(memory2.grow(&mut store, 0).is_ok());
124 | 
125 |     Ok(())
126 | }
```

crates/rebeldb-wasm/src/lib.rs
```
1 | // RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
2 | //
3 | // reboldb_wasm::
4 | 
5 | pub mod runtime;
```

crates/rebeldb-wasm/src/runtime.rs
```
1 | //
2 | 
3 | // crates/wasmtime-runtime/src/lib.rs
4 | use rebeldb::runtime::{
5 |     Result, RuntimeConfig, WasmError, WasmInstance, WasmMemory, WasmRuntime, WasmValue,
6 | };
7 | use wasmtime::{Engine, Instance, Memory, Module, Store};
8 | 
9 | pub struct WasmtimeRuntime {
10 |     store: Store<()>,
11 |     engine: Engine,
12 | }
13 | 
14 | pub struct WasmtimeInstance {
15 |     instance: Instance,
16 |     store: Store<()>,
17 | }
18 | 
19 | pub struct WasmtimeMemory<'a> {
20 |     memory: Memory,
21 |     store: &'a mut Store<()>,
22 | }
23 | 
24 | impl WasmtimeRuntime {
25 |     pub fn new() -> Self {
26 |         let engine = Engine::default();
27 |         let store = Store::new(&engine, ());
28 |         Self { store, engine }
29 |     }
30 | }
31 | 
32 | impl WasmRuntime for WasmtimeRuntime {
33 |     fn instantiate_module(&mut self, wasm_bytes: &[u8]) -> Result<Box<dyn WasmInstance>> {
34 |         let module = Module::new(&self.engine, wasm_bytes)
35 |             .map_err(|e| WasmError::Instantiation(e.to_string()))?;
36 | 
37 |         let instance = Instance::new(&mut self.store, &module, &[])
38 |             .map_err(|e| WasmError::Instantiation(e.to_string()))?;
39 | 
40 |         Ok(Box::new(WasmtimeInstance {
41 |             instance,
42 |             store: Store::new(&self.engine, ()),
43 |         }))
44 |     }
45 | 
46 |     fn with_config(config: RuntimeConfig) -> Result<Self> {
47 |         let mut wt_config = wasmtime::Config::new();
48 | 
49 |         if config.enable_threads {
50 |             wt_config.wasm_threads(true);
51 |         }
52 |         if config.enable_simd {
53 |             wt_config.wasm_simd(true);
54 |         }
55 | 
56 |         let engine =
57 |             Engine::new(&wt_config).map_err(|e| WasmError::Instantiation(e.to_string()))?;
58 |         let store = Store::new(&engine, ());
59 | 
60 |         Ok(Self { store, engine })
61 |     }
62 | }
63 | 
64 | impl WasmInstance for WasmtimeInstance {
65 |     fn get_memory(&mut self, name: &str) -> Result<Box<dyn WasmMemory + '_>> {
66 |         let memory = self
67 |             .instance
68 |             .get_memory(&mut self.store, name)
69 |             .ok_or_else(|| WasmError::Memory(format!("Memory '{}' not found", name)))?;
70 | 
71 |         Ok(Box::new(WasmtimeMemory {
72 |             memory,
73 |             store: &mut self.store,
74 |         }))
75 |     }
76 |     fn call_function(&mut self, name: &str, params: &[WasmValue]) -> Result<Vec<WasmValue>> {
77 |         let func = self
78 |             .instance
79 |             .get_func(&mut self.store, name)
80 |             .ok_or_else(|| WasmError::FunctionNotFound(name.to_string()))?;
81 | 
82 |         // Convert WasmValue to wasmtime::Val
83 |         let params: Vec<wasmtime::Val> = params
84 |             .iter()
85 |             .map(|v| match v {
86 |                 WasmValue::I32(x) => wasmtime::Val::I32(*x),
87 |                 WasmValue::I64(x) => wasmtime::Val::I64(*x),
88 |                 WasmValue::F32(x) => wasmtime::Val::F32(x.to_bits()),
89 |                 WasmValue::F64(x) => wasmtime::Val::F64(x.to_bits()),
90 |             })
91 |             .collect();
92 | 
93 |         let mut results = vec![wasmtime::Val::I32(0); func.ty(&self.store).results().len()];
94 | 
95 |         func.call(&mut self.store, &params, &mut results)
96 |             .map_err(|e| WasmError::Runtime(e.to_string()))?;
97 | 
98 |         // Convert back to our WasmValue
99 |         Ok(results
100 |             .into_iter()
101 |             .map(|v| match v {
102 |                 wasmtime::Val::I32(x) => WasmValue::I32(x),
103 |                 wasmtime::Val::I64(x) => WasmValue::I64(x),
104 |                 wasmtime::Val::F32(x) => WasmValue::F32(f32::from_bits(x)),
105 |                 wasmtime::Val::F64(x) => WasmValue::F64(f64::from_bits(x)),
106 |                 _ => unreachable!(),
107 |             })
108 |             .collect())
109 |     }
110 | }
111 | 
112 | impl WasmMemory for WasmtimeMemory<'_> {
113 |     fn size(&self) -> usize {
114 |         self.memory.size(&self.store) as usize * 65536 // Convert pages to bytes
115 |     }
116 | 
117 |     fn grow(&mut self, pages: u32) -> Result<()> {
118 |         self.memory
119 |             .grow(&mut self.store, u64::from(pages))
120 |             .map(|_| ()) // Ignore the returned size
121 |             .map_err(|e| WasmError::Memory(e.to_string()))
122 |     }
123 | 
124 |     fn read(&self, offset: usize, buf: &mut [u8]) -> Result<()> {
125 |         let data = self.memory.data(&self.store);
126 |         if offset + buf.len() > data.len() {
127 |             return Err(WasmError::Memory("Read outside memory bounds".into()));
128 |         }
129 |         buf.copy_from_slice(&data[offset..offset + buf.len()]);
130 |         Ok(())
131 |     }
132 | 
133 |     fn write(&mut self, offset: usize, data: &[u8]) -> Result<()> {
134 |         let mem_data = self.memory.data_mut(&mut self.store);
135 |         if offset + data.len() > mem_data.len() {
136 |             return Err(WasmError::Memory("Write outside memory bounds".into()));
137 |         }
138 |         mem_data[offset..offset + data.len()].copy_from_slice(data);
139 |         Ok(())
140 |     }
141 | }
```


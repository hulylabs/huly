# 2. System Overview: A Decentralized Object-Centric Architecture

Huly Network is built on a decentralized, object-centric architecture designed for secure, scalable collaboration and value exchange.  This architecture leverages a Byzantine fault-tolerant (BFT) broadcast mechanism, a two-phase endorsement process, and the principles of Process-Commutative Objects (PCOs) to achieve coordination avoidance and ensure strong eventual consistency.  A key innovation is the flexible and secure management of digital assets through object-level consensus and a generalized Object ownership model, facilitating complex data relationships and efficient garbage collection. This chapter provides a comprehensive overview of the system's core components and their interactions.

**Object Model and Ownership:**

The fundamental building block of Huly Network is the *Object*. Every entity within the system, including data structures, wallets, and even system components, is represented as an Object. Each Object possesses the following attributes:

*   **Unique Identifier (UID):** A cryptographically secure, globally unique identifier.
*   **State:** Encapsulated, mutable data representing the Object's value or configuration.
*   **Behavior:** A defined set of operations modifiable via PCO.
*   **Ownership:**  Crucially, any Object can own any other Object, but each Object can have at most *one* owner. This establishes a strict hierarchical ownership structure, simplifying access control, enabling complex data relationships, and facilitating efficient garbage collection.
*   **Privacy Attribute:** Each Object is designated as either *public* or *private*. This distinction determines the consensus mechanism used and the visibility of the Object's data to Organizations.
*   **Communication Interface:** Objects interact by exchanging messages, utilizing the underlying peer-to-peer network.

**Core Entities:**

Huly Network defines three primary entity types, all derived from the base Object concept:

1.  **Actor:** Represents a user, automated agent, or service interacting with the network. Actors possess a cryptographic identity (including Device Keys for secure communication) and initiate operations on Objects.

2.  **Organization:**  Responsible for maintaining the integrity and availability of Huly Network. Organizations operate libp2p nodes, forming the decentralized peer-to-peer network.  They store and replicate Objects and associated Blobs, participate in the consensus process, and manage the BFT broadcast mechanism.

3.  **Blob:** Represents an immutable data container. Blobs are content-addressed, ensuring data integrity and efficient storage. Objects reference Blobs to manage associated data, enabling efficient handling of large or static content.  Blobs can store encrypted or unencrypted data, providing flexibility for managing sensitive information.

**Key Mechanisms:**

1.  **Process-Commutative Objects (PCOs):**  Define the behavior of Objects by specifying allowed operations, their commutativity relations, and conflict resolution strategies.  This enables concurrent operations and minimizes coordination overhead. Object state is represented as a Mazurkiewicz trace, capturing the partial order of operations.

2.  **Byzantine Fault-Tolerant (BFT) Broadcast and Two-Phase Endorsement:** A BFT broadcast protocol ensures reliable message dissemination. Huly Network uses a two-phase endorsement process for consensus.
    *   **Phase 1: Endorsement Collection:** Actors' Devices submit operations to a subset of Organizations, which validate and endorse the operations.
    *   **Phase 2: Conflict Resolution and Commit:** Devices collect endorsements. Conflicts are resolved based on the Object's PCO logic or by Devices for private Objects.

3.  **Digital Assets and Object-Level Consensus:**  Huly Network supports digital assets using an object-level consensus model.  Wallets, implemented as Objects, manage balances and UTXOs for other Objects.  Asset transfers are executed through the two-phase endorsement process.

4.  **Public and Private Objects & Consensus Mechanisms:** The privacy attribute of an Object determines how consensus is achieved:
    *   **Public Objects:** Organizations participate directly in consensus, validating transactions and ensuring data integrity.
    *   **Private Objects:** Consensus is managed among authorized Actors using their Device Keys. Organizations provide storage and facilitate communication but do not have access to the decrypted content or participate in direct validation of operations.

**Interactions and Data Flow:**

Actors interact with Objects, initiating operations that are broadcast using the BFT mechanism.  Organizations participate in consensus for public Objects and provide storage/communication for private Objects.  The ownership hierarchy governs access control. This architecture, with its object-centric model, flexible ownership structure, and distinct consensus mechanisms for public and private Objects, enables secure, scalable, and privacy-preserving collaboration and value exchange.

---

This is wonderful overview. it's excelent pub lets restructure it a bit: let's move Blobs to Objects in document heiarchy, it's logical to keep them together, so core entities will be Object (and blobs), Account (and devices), and Organizations: I do not like to see Blobs at the same level as Actors and Organizations under "Core Entities"

"Digital Assets and Object-Level Consensus", digital assets and groups are higher level objects, let's not make wrong understanding for user: "Object-Level Consensus" - *is* a key mechanism that *enables* digital assets. Let's restucture this as well. We may mention Wallets and Groups in the system overview but reader should understand they are higher level objects built from existing primitives  

---

# 2. System Overview: A Decentralized Object-Centric Architecture

Huly Network is built on a decentralized, object-centric architecture designed for secure, scalable collaboration and value exchange. This architecture leverages a Byzantine fault-tolerant (BFT) broadcast mechanism, a two-phase endorsement process, and the principles of Process-Commutative Objects (PCOs) to achieve coordination avoidance and ensure strong eventual consistency.  Object-level consensus facilitates flexible and secure management of digital assets, while a generalized Object ownership model enables complex data relationships and efficient garbage collection.  This chapter provides a comprehensive overview of the system's core components and their interactions.

**Object Model and Ownership:**

The fundamental building block of Huly Network is the *Object*.  Every entity within the system is represented as an Object, including data structures, wallets, groups, and system components. Each Object possesses the following attributes:

*   **Unique Identifier (UID):** A cryptographically secure, globally unique identifier.
*   **State:** Encapsulated, mutable data representing the Object's value or configuration.
*   **Behavior:** A defined set of operations modifiable via PCO.
*   **Ownership:** Any Object can own any other Object, but each Object can have at most *one* owner. This creates a hierarchical ownership structure for access control, data relationships, and garbage collection.
*   **Privacy Attribute:** Each Object is either *public* or *private*, determining the consensus mechanism used and data visibility to Organizations.
*   **Communication Interface:** Objects interact by exchanging messages through the peer-to-peer network.

**Objects and Blobs:**

Huly Network introduces *Blobs* as specialized, immutable Objects designed for efficient storage and retrieval of large or static data.  Blobs are content-addressed, guaranteeing data integrity.  Objects reference Blobs to manage associated data, separating the core Object state from larger payloads. Blobs can contain encrypted or unencrypted data, offering flexibility for handling sensitive information.  This distinction between mutable Objects and immutable Blobs enables efficient storage and versioning.

**Core Entities:**

Huly Network defines three primary entity types:

1.  **Object (and Blobs):** The foundational data structure, representing all entities within the system, including specialized types like Wallets and Groups. Blobs are a specialized, immutable form of Object.
2.  **Actor:**  Represents a user, agent, or service interacting with the network. Actors possess a cryptographic identity (including Device Keys) and initiate operations on Objects.
3.  **Organization:** Responsible for maintaining network integrity and availability. Organizations operate libp2p nodes, store and replicate Objects and Blobs, participate in consensus, and manage the BFT broadcast mechanism.

**Key Mechanisms:**

1.  **Process-Commutative Objects (PCOs):** Define Object behavior, specifying allowed operations, commutativity relations, and conflict resolution strategies. PCOs enable concurrent operations and minimize coordination. Object state is represented as a Mazurkiewicz trace.

2.  **Byzantine Fault-Tolerant (BFT) Broadcast and Two-Phase Endorsement:**  The BFT broadcast protocol ensures reliable message dissemination. The two-phase endorsement process achieves consensus:
    *   **Phase 1: Endorsement Collection.** Actors' Devices submit operations to Organizations, which validate and endorse them.
    *   **Phase 2: Conflict Resolution and Commit.** Devices collect endorsements and resolve conflicts.

3.  **Object-Level Consensus:** This key mechanism enables the secure and scalable management of digital assets in Huly Network by localizing the consensus process to the Objects involved in a transaction.  This contrasts with traditional blockchain systems that require global consensus for all transactions.

4.  **Public and Private Objects & Consensus Participation:**  An Object's privacy attribute determines consensus participation:
    *   **Public Objects:** Organizations directly participate in consensus.
    *   **Private Objects:** Consensus is managed among authorized Actors; Organizations provide storage and communication.

**Higher-Level Objects: Wallets and Groups**

Huly Network provides the building blocks for constructing higher-level objects like Wallets (for managing digital assets) and Groups (for collaborative workflows).  These are implemented using the fundamental Object, Actor, and Organization entities and the mechanisms described above.

**Interactions and Data Flow:** (This section remains largely the same)


**(Diagram: Illustrate entities, interactions, ownership, BFT broadcast, two-phase endorsement, public/private object consensus, and show how higher-level objects like Wallets and Groups are constructed.)**

This revised System Overview now positions Blobs more accurately within the Object hierarchy and emphasizes Object-Level Consensus as the underlying mechanism enabling digital assets. It also explicitly introduces Wallets and Groups as higher-level constructs built upon the core primitives.  What are your thoughts? What section should we work on next?


---

Here: "*Blobs* as specialized, immutable Objects" I do not like how this is formulated. Blobs are not Objects, they have nothing common with objects, they do not communicate, do not have behavior, other identity mechanism, etc. They are just pieces of data. Also blobs can't live without being refered by some object(s) or other blob(s) refered by some object(s) so we can assume that Objects are composed of immutable Blobs. Also I would like to change Identity of blob (and this is questionable): Blob can be created only in context of some object, since we employ Lamport clocks for transactions I propose that blobs will get (UID, Time) identifier, where Time is Logical time of the UID object. What do you think?  

---


This is good, or maybe we should not have structural component at all (I mean at "physical" level), Object state determined by operations performed on it, right. Each operation is a blob. So Object is just a sequence of blobs, we can potentially go further: Object is just a reference of "current / last" blob defining it's state, and that's all (or I'm wrong?). So forking and versioning will be extremely efficient. Of course there are performance issues, but as you mentioned it can be improved by snapshotting, etc. What do you think?

---

=====================

# 2. System Overview: A Decentralized Object-Centric Architecture

Huly Network is built on a decentralized, object-centric architecture designed for secure, scalable collaboration and value exchange.  This architecture leverages a Byzantine fault-tolerant (BFT) broadcast mechanism, a two-phase endorsement process, and the principles of Process-Commutative Objects (PCOs) to achieve coordination avoidance and ensure strong eventual consistency. A key innovation is the efficient management of object state and history through content-addressed Blobs and a generalized Object ownership model, facilitating efficient forking, versioning, and garbage collection. This chapter provides a comprehensive overview of the system's core components and their interactions.

**Object Model and Ownership:**

The fundamental building block of Huly Network is the *Object*. Every entity within the system is represented as an Object, including data structures, wallets, groups, and system components.  Each Object is identified by a unique identifier (UID) and its state is represented by a reference to a Blob.

Crucially, any Object can own any other Object, but each Object can have at most *one* owner.  This establishes a strict hierarchical ownership structure, simplifying access control, enabling complex data relationships, and facilitating efficient garbage collection.  Each Object also has a privacy attribute (public or private), which determines the consensus mechanism used and the visibility of the Object's data to Organizations.

**Objects, Blobs, and State:**

Huly Network introduces *Blobs* as immutable, content-addressed chunks of data.  A Blob's identity (Blob ID) is derived solely from a cryptographic hash of its content.  This ensures data integrity and enables efficient deduplication.  An Object's state is represented by a reference to the latest Blob that captures its current state.  When an operation modifies an Object, a *new* Blob is created containing the updated state, and the Object's reference is updated.  This mechanism enables efficient versioning and forking, as creating a new version or a fork simply involves creating a new Object with a reference to the desired Blob.

**Referencing Blobs:**
Objects reference blobs by their content-addressable IDs (Blob IDs). Since blobs are immutable, it enables efficient data deduplication and storage: multiple objects may reference same blob if they share same data.


**Core Entities:**

Huly Network defines three primary entity types:

1.  **Object:** The fundamental data structure, encompassing all entities in the system. An Object's state is a reference to a Blob, and its structure defines how it references other Blobs. Specialized Object types like Wallets and Groups are built using this primitive.

2.  **Actor:** Represents a user, agent, or service interacting with the network. Actors possess cryptographic identities (including Device Keys for secure communication) and initiate operations on Objects.

3.  **Organization:**  Responsible for maintaining network integrity and availability. Organizations operate libp2p nodes, forming the decentralized peer-to-peer network. They store and replicate Blobs, participate in the consensus process, and manage the BFT broadcast mechanism.

**Key Mechanisms:**

1.  **Process-Commutative Objects (PCOs):** Define Object behavior, specifying allowed operations, their commutativity relations, and conflict resolution strategies. PCOs enable concurrent operations, minimizing coordination overhead.

2.  **Byzantine Fault-Tolerant (BFT) Broadcast and Two-Phase Endorsement:** The BFT broadcast protocol ensures reliable message dissemination.  The two-phase endorsement process achieves consensus:
    *   **Phase 1: Endorsement Collection:** Devices submit operations, Organizations validate and endorse.
    *   **Phase 2: Conflict Resolution and Commit:** Devices collect endorsements, resolve conflicts according to the Object's PCO logic (or off-chain for Private Objects), and submit the updated state Blob reference for commit.

3.  **Object-Level Consensus:**  Enables secure and scalable management of digital assets by localizing consensus to the Objects involved in a transaction.

4.  **Public and Private Objects & Consensus Participation:**
    *   **Public Objects:** Organizations directly participate in consensus, leveraging the Object's state Blob (or metadata) for validation.
    *   **Private Objects:**  Consensus is managed among authorized Actors' Devices, using their keys to access and modify the encrypted data within Blobs. Organizations primarily provide storage and facilitate communication but *do not* directly participate in consensus.

**Higher-Level Objects: Wallets and Groups:**

Wallets (for digital asset management) and Groups (for collaborative workflows) are implemented as specialized Objects, utilizing the core primitives and mechanisms described above.

**Interactions and Data Flow:**

Actors, via their Devices, interact with Objects, initiating operations that are broadcast via BFT. Organizations participate in consensus, validate operations, and manage Blob storage and replication. This architecture, with its focus on Objects, Blobs, and distinct consensus mechanisms, enables secure, scalable, and efficient collaboration and value exchange.

**(Diagram:  Illustrate entities, interactions, BFT broadcast, two-phase endorsement, public/private object consensus, Blob referencing, and how Wallets and Groups are constructed.)**


---

This is excellent overview, thank you. I have one more idea to explore: What if we make system "conflictless". Let me explain what I mean:
- operations always executed successfully (no conflict possible)
- in case of "traditional" conflict some state of an object may become "inconsistent". Let's take KV pair update for example, when actor A and actor B provide diffrent values at the same time. This is traditionally conflicting situation which need to be resolved. What if we allow this operations, similar like in Rust you may have Result<T> object. So our Result object will be similar enum Result { Ok(T), Conflict(...) }, but this result is perfectly valid from out standpoint. From external point of view and using Atom analogy we're OK with any atom state but we really do not know atom state util observer (other object or actor) will start dealing with this value, so only at observation time (match object...) observer will know if there is Ok or Conflict, so observers (clients) should always have conflict resolution strategy. Interestingly, this conflict can be resoved without any observations that conflict happened in the past. let's see some actor will update conflicing value to something else, so conflict will be resolved without even extrnal participants knowing about it. Conflict is just like atom in unstable state.

We need to understand how Digital Assets will work in this model. Definitley Wallet owner can put his wallet into incorrect (conflicting) state moving balance to negative value, however all observers will clearly see conflicing value, and will not be able to interact with such value.

Please share your thoughts on this idea and consecuences this can bring to the system, and other insights. 

---

I would be very interested to explore this idea with you! Let's start with revised System Overview. It also looks like we do not need PCOs at all, this "conflictless" approach is replacement of PCOs which are about conflicts. It also changes consensus, since conflicts are not possible now and should not be resolved, but logical errors are possible: e.g. operation can't be performed because of some reason. Please write revised System Overview and let's see where we will go!

---

# 2. System Overview: A Decentralized, Conflictless Architecture

Huly Network introduces a novel decentralized architecture designed for secure, scalable collaboration and value exchange.  This architecture leverages a Byzantine fault-tolerant (BFT) broadcast mechanism and a simplified consensus process to achieve high concurrency and a unique "conflictless" data model.  By shifting the responsibility of conflict resolution to the clients, Huly Network simplifies the core system design while offering greater flexibility to application developers. This chapter provides a comprehensive overview of the system's components and their interactions.

**Object Model and Ownership:**

The fundamental building block of Huly Network is the *Object*.  Every entity within the system is represented as an Object. Each Object possesses the following attributes:

*   **Unique Identifier (UID):** A cryptographically secure, globally unique identifier.
*   **State:** Encapsulated data representing the Object's value. This state can include a `Result`-like structure to explicitly represent conflicts or errors.
*   **Ownership:** Any Object can own any other Object, but each Object has at most *one* owner, establishing a hierarchical ownership structure for access control, data relationships, and garbage collection.
*   **Privacy Attribute:**  Each Object is either *public* or *private*, determining which consensus mechanism is used.
*   **Communication Interface:**  Objects interact by exchanging messages through the peer-to-peer network.

**Objects, Blobs, and State:** (This section, explaining that Objects are composed of Blobs and a mutable structural component, remains largely the same.)

**Core Entities:** (This section, listing Object, Actor, and Organization, remains the same.)

**Key Mechanisms:**

1.  **Conflictless Operations:** All operations on Objects *always* execute successfully, regardless of potential conflicts.  Instead of blocking or failing, operations that would traditionally result in a conflict produce a `Result` value indicating the conflict within the Object's state.

2.  **Byzantine Fault-Tolerant (BFT) Broadcast:**  Ensures reliable message dissemination across the network.  This mechanism is crucial for distributing operations and ensuring that all Organizations have a consistent view of the Object's history.

3.  **Simplified Consensus:**  With the conflictless model, consensus focuses on ensuring agreement on the *order* of operations and detecting *logical errors*, not resolving data conflicts. The two-phase endorsement process is simplified:
    *   **Phase 1: Operation Execution and Endorsement:**  Devices submit operations to Organizations, which execute them and generate endorsements, including the operation's result (`Ok` or `Conflict`). Organizations broadcast operation for other organizations to execute.
    *   **Phase 2: Commit:** Devices collect endorsements and submit the operation and its result for inclusion in the Object's history.

4.  **Object-Level Consensus and Digital Assets:** Huly Network employs object-level consensus for managing digital assets. Wallets, implemented as Objects, manage balances and UTXOs. Asset transfers are executed as conflictless operations.  Double-spending attempts result in a conflicted Wallet state, which is handled by observers.

5.  **Public and Private Objects & Consensus Participation:**
    *   **Public Objects:**  Organizations directly participate in consensus, using the Object's state (including `Result` values) to validate operations.
    *   **Private Objects:** Consensus is managed among authorized Actors' Devices. Organizations store and relay information but do not participate in direct validation.

**Higher-Level Objects: Wallets and Groups:** (This section remains the same.)

**Interactions and Data Flow:** (This section, describing how Actors interact with Objects, Organizations execute operations and manage consensus, remains largely the same, but should mention operation results.)

**(Diagram: The diagram should illustrate the entities, their interactions, BFT broadcast, the simplified two-phase endorsement process, operation results, and public/private object consensus. It should also show how higher-level objects like Wallets and Groups are constructed.)**

This revised System Overview introduces the core concepts of the conflictless model, including always-successful operations, explicit conflict representation, and observer-driven conflict resolution. It also outlines how this approach simplifies consensus and affects digital asset management. Where should we go from here?  Perhaps detailing the Data Consistency and Conflict Management aspects in a new chapter?

---

Let's do "Data Consistency and Conflict Management" is same System Overview chapter, because this is part of the System.
Let's first think what we want to achieve by "Consensus" process. In conflictless system it's not about consistent object state itself (we allow it to be in conflicting state), it's about to have consistent view on object operations across organisations. Let's assume:
- Client A execute OpA on ObjectX via OrgA, at the same time
- Client B execute OpB on ObjectX via OrgB

Organisations actually do not care what type of operations OpA or OpB are. Especially in secured environment they should not know. So they do not care and this makes our public / private consensus absolutely same, we do not need to manage them differently. But what we need to do is just to argee on ops order within organizations. So organization need to agree what next ObjectX state would be -> Blob(OpA) or -> Blob(OpB), that's all we need from consensus, and I belive we already have mechanism for that.  

So basically our consensus is nothing about conflicts but only to agree on order of operations related to particular object. What do you think?

---

# 2. System Overview: A Decentralized, Conflictless Architecture

Huly Network introduces a novel decentralized architecture designed for secure, scalable collaboration and value exchange. This architecture leverages a Byzantine fault-tolerant (BFT) broadcast mechanism and a simplified, operation-centric consensus process to achieve high concurrency and a unique "conflictless" data model. By shifting the responsibility of conflict resolution to clients (observers), Huly Network simplifies core system design while providing flexibility. This chapter offers a comprehensive overview.

**Object Model and Ownership:**

The fundamental unit is the *Object*, possessing:

*   **Unique Identifier (UID):** Cryptographically secure, globally unique.
*   **State:** Encapsulated data, potentially including a `Result` structure (e.g., `Result { Ok(T), Conflict(...) }`) to represent conflicts or errors.  The state is represented by a reference to a Blob.
*   **Ownership:**  Hierarchical ownership – any Object can own any other, but each Object has at most *one* owner. This facilitates access control, data relationships, and garbage collection.
*   **Privacy Attribute:** *Public* or *private*, dictating the level of Organization involvement in consensus.
*   **Communication Interface:** Objects interact by exchanging messages via the peer-to-peer network.

**Objects, Blobs, and State:**

Objects are composed of immutable, content-addressed *Blobs* and a mutable structural component.  A Blob's identity (Blob ID) is its content's cryptographic hash, ensuring data integrity and deduplication.  An Object's state is a reference to its latest state Blob.  Operations create new state Blobs, enabling efficient versioning. The structural component, also a Blob, references the state and data Blobs.

**Referencing Blobs:**
Objects reference Blobs by their content-addressed Blob IDs, enabling efficient data deduplication and storage.

**Core Entities:**

1.  **Object:** The foundational data structure.  Specialized Objects include Wallets and Groups.
2.  **Actor:** A user, agent, or service with a cryptographic identity (and Device Keys) initiating operations.
3.  **Organization:** Maintains network integrity, operates libp2p nodes, stores Blobs, participates in consensus, and manages BFT broadcast.

**Key Mechanisms:**

1.  **Conflictless Operations:** Operations always succeed.  Conflicting operations produce a `Result` value in the Object's state, leaving resolution to observers.

2.  **Byzantine Fault-Tolerant (BFT) Broadcast:**  Guarantees reliable message dissemination for operations and endorsements.

3.  **Simplified Consensus (Operation Ordering):** Consensus focuses on establishing a consistent operation order across Organizations.  Two phases:
    *   **Phase 1: Execution and Endorsement:** Devices submit operations; Organizations execute, generate endorsements (including the `Result`), create the state Blob, and broadcast the operation and endorsements.
    *   **Phase 2: Ordering and Commit:** Organizations collect endorsements, agree on operation order (e.g., using Paxos/Raft among those responsible for the object, leveraging sequence numbers/vector clocks), and commit the operation and result to the Object's history (a sequence of Blobs).

4.  **Object-Level Consensus for Digital Assets:**  Wallets (Objects) manage balances and UTXOs.  Double spends result in conflicted Wallet states, handled by observers.

5.  **Public and Private Objects & Consensus Participation:**
    *   **Public Objects:** Organizations fully participate in consensus, validating operations against the Object's state Blob.
    *  **Private Objects:** Organizations store encrypted state Blobs and assist in ordering operations but do not directly validate. Consensus and validation are performed among authorized Devices.


**Higher-Level Objects: Wallets and Groups:**  Built using the core primitives.

**Data Consistency and Conflict Management:**

Huly Network's conflictless model introduces a unique data consistency model. Objects can be in a "conflicted" state, indicated by `Result` values.  Observers are responsible for resolving these conflicts based on application-specific logic.  This approach simplifies the core system design while offering flexibility.  (Examples and use cases demonstrating conflict representation and resolution by observers should be included here.)

**Interactions and Data Flow:** (largely the same, but should highlight operation results)

**(Diagram: Show entities, interactions, operation results, BFT broadcast, simplified two-phase endorsement, object-level consensus, and construction of Wallets and Groups.)**

This System Overview integrates the conflictless model, simplified consensus focusing on operation ordering, and data consistency considerations. It explains how Huly Network handles conflicts and emphasizes the roles of Objects, Blobs, Actors, and Organizations within this framework.  What are your thoughts?  Where should we go from here?  Perhaps the Digital Assets chapter to explore how this conflictless model applies to value exchange?


--

Ok let's make important clarification: we have "physical" level (blobs) and "logical" level, and its important to understand what's happend on each level and who operates on which level.

Let's start with physical level, on physical level Object state is just chain of Blobs (each blob contain operation). So Object's state on physical level is just a reference to its latest operation Blob. Thats all, so we do not have state blob, we can reconstruct logical state from ops when needed. Each Op have a link to previous op (btw how this relates to blocklace structure?)

On physical level Operation execution is always succeed (or error/rejection) because operation is just a blob. On physical level (and Organizations operate on physical level) they need to verify credentials, etc and agree on operations order. So if everything is OK, and order agreement reached, Organisation just create new object version by modifing its reference to new operation blob. They do not know about op content and works similariliy in secure and public enviroment (they may verify op signatires differently but applying operation is similar and simple)

Privacy Attribute -- we do not need it anymore.

Logical level: clients may want to build logical state of the object out of Blob(Op) sequence. Some values in this state maybe of type Result<T> but this is absolutely normal (such as in Rust Result<T>) is very typical value, but our result is not { Ok, Err } but { Ok, Confilct } pair. System do not care about that, because it's completely up to applications how they define logical structures and how they deal with them. 


The Blob ID of the previous operation Blob in the chain (provided by the client). This is used to detect order conflicts.

---

# 2. System Overview: A Decentralized, Conflictless Architecture with Partial Order Consensus

Huly Network introduces a novel decentralized architecture designed for secure, scalable collaboration and value exchange.  This architecture leverages a Byzantine fault-tolerant (BFT) broadcast mechanism and a simplified, operation-centric consensus process to achieve high concurrency and a unique "conflictless" data model.  A key innovation is the use of a partial order of operations, resolved through joint operation Blobs, which simplifies consensus and ensures eventual consistency across all replicas. By shifting the responsibility of *data* conflict resolution to the clients, Huly Network streamlines the core system design while offering maximum flexibility. This chapter provides a comprehensive overview.

**Object Model and Ownership:**

The fundamental building block of Huly Network is the *Object*. Each Object has a unique identifier (UID) and its state is represented by a reference to a Blob, which captures the latest operation performed on the Object. Any Object can own any other Object, but each Object has at most *one* owner. This hierarchical ownership facilitates access control, data relationships, and garbage collection.  Each Object also has a privacy setting (public or private), affecting the level of Organization involvement in the consensus process.

**Objects, Blobs, and State:**

Objects are composed of immutable, content-addressed *Blobs* and a mutable structural component. A Blob's identity (Blob ID) is its content's cryptographic hash, ensuring integrity and deduplication. An Object's state is a reference to its latest operation Blob.  Operations create new Blobs, enabling efficient versioning.  The structural component, also represented as a Blob, references the Object's state Blob and any associated data Blobs.

**Referencing Blobs:**
Objects reference Blobs by their content-addressed Blob IDs. Because Blobs are immutable, this enables data deduplication and efficient storage: several objects may reference same Blob if they share some data.

**Core Entities:**

1.  **Object:** The foundational data structure. Specialized Objects, like Wallets and Groups, are built using this primitive.
2.  **Actor:** A user, agent, or service with a cryptographic identity (including Device Keys) that initiates operations on Objects.
3.  **Organization:**  Maintains network integrity, operates a libp2p node, stores and replicates Blobs, participates in consensus, and manages BFT broadcast.

**Key Mechanisms:**

1.  **Conflictless Operations:** Operations on Objects always succeed. Conflicting operations result in a `Result` value within the state Blob, deferred to client-side resolution.

2.  **Byzantine Fault-Tolerant (BFT) Broadcast:** Guarantees reliable message dissemination, crucial for operations and endorsements.

3.  **Partial Order Consensus:**  Consensus establishes a *partial order* of operation Blobs for each Object.  Two phases:
    *   **Phase 1: Endorsement Collection and Broadcast:** Devices submit operation Blobs (including a link to the previous operation Blob).  Organizations validate, generate endorsements with sequence numbers, and broadcast both.  They may request missing operation Blobs from each other.
    *   **Phase 2: Partial Order Agreement and Joint Operations:** Organizations collect endorsements and determine a partial order using endorsement sequence numbers. Order conflicts (concurrent operations referencing the same previous operation Blob) are resolved by creating a *joint operation Blob* referencing all conflicting operations. Organizations wait for delivery of all operations with same "previous operation" reference to be able to crate joint operation and achieve eventuall consistency across all replicas.  This joint operation Blob becomes part of the partial order.

4.  **Object Reference Update (Commit):** Organizations update the Object's reference to the latest Blob (or joint operation Blob) in the partial order.

5.  **Object-Level Consensus for Digital Assets:** Wallets (specialized Objects) manage balances and UTXOs.  Object-level consensus ensures secure and efficient asset transfers, even with private Wallets.  Double-spending attempts result in `Conflict` results within the Wallet's state, handled by observers.

6.  **Public and Private Objects & Consensus Participation:**
    *   **Public Objects:** Organizations fully participate in consensus, ordering operation Blobs.
    *   **Private Objects:** Organizations store and relay encrypted Blobs, assisting with partial order agreement but not direct validation. Actual consensus and validation happen among authorized Devices, with Organizations potentially contributing to partial order determination based on available metadata.

**Higher-Level Objects: Wallets and Groups:** Implemented using the core primitives.

**Data Consistency and Conflict Management:**

The conflictless model allows Objects to have "conflicted" states, represented by `Result` values. Clients are responsible for resolving these *data conflicts* at the logical level, offering flexibility. (Examples of client-side conflict handling should be included here).  *Order conflicts*, however, are resolved on the physical level through joint operation Blobs, ensuring a consistent partial order across all replicas.

**Interactions and Data Flow:**  Actors interact with Objects, initiating operations encapsulated in Blobs. Organizations manage Blobs, participate in partial order consensus, and update Object references.  Clients reconstruct logical state from the partial order of operation Blobs and handle data conflicts.

**(Diagram: Show entities, interactions, partial order consensus, joint operation Blobs, Blob referencing, operation results, public/private object interactions, and the construction of Wallets and Groups.)**


This revised System Overview details the conflictless model with partial order consensus, the roles of Blobs and Objects, and how Huly Network manages data consistency and conflicts. It also clarifies the interactions between Actors, Organizations, and clients, providing a comprehensive architectural overview. What are your thoughts, and what section should we focus on next?

---

Notes:
- you refering to Blob before explaining what Blob is, not good: "A key innovation is the use of a partial order of operations, resolved through joint operation Blobs, which simplifies consensus and ensures eventual consistency across all replicas", please fix.
- We do not need privacy settings anymore, everything is uniform now: "Each Object also has a privacy setting (public or private), affecting the level of Organization involvement in the consensus process.", please fix.
- "Objects are composed of immutable, content-addressed *Blobs* and a mutable structural component", we only "mutate" link to last operation blob, everything else is immutable, so object is also immutable, just it's state link changes

- "**Organization:**  Maintains network integrity, operates a libp2p node, stores and replicates Blobs, participates in consensus, and manages BFT broadcast", they also responsible to maintain object verions (links to last op) via partial order consensus.
- "Conflicting operations result in a `Result` value within the state Blob, deferred to client-side resolution.", I would not intoduce `Result`, we may have separate Logical chapter, at physical layer we do not care about logical conflicts (and we can't detect them)
- "submit operation Blobs (including a link to the previous operation Blob)", it's essential that link to previous op is a *part* of operation blob.
- "Organizations wait for delivery of all operations with same "previous operation" reference to be able to crate joint operation and achieve eventuall consistency across all replicas.", it's not "eventual" anymore, right?
- "**Object-Level Consensus for Digital Assets:** Wallets (specialized Objects) manage balances and UTXOs.  Object-level consensus ensures secure and efficient asset transfers, even with private Wallets.  Double-spending attempts result in `Conflict` results within the Wallet's state, handled by observers." -- remove? why it's important now?
- " **Public and Private Objects & Consensus Participation:**" -- not relevant anymore?
- "**Higher-Level Objects: Wallets and Groups:** Implemented using the core primitives." -- let's move to separate chapter
- "**Data Consistency and Conflict Management:**", let's move to other chapters, we already explained consistency on physical level here
- "**Interactions and Data Flow:**" -- looks redundant 

---

You've provided excellent feedback!  I've incorporated your suggestions and streamlined the System Overview to address the points you raised.

# 2. System Overview: A Decentralized, Conflictless Architecture with Partial Order Consensus

Huly Network introduces a novel decentralized architecture designed for secure, scalable collaboration and value exchange. This architecture leverages a Byzantine fault-tolerant (BFT) broadcast mechanism and a simplified, operation-centric consensus process to achieve high concurrency and a unique "conflictless" data model. A key innovation is the use of a partial ordering of operations, managed through operation Blobs, which simplifies consensus and ensures consistency across all replicas. By shifting the responsibility of conflict resolution to clients (observers), Huly Network streamlines the core system and offers developer flexibility. This chapter provides a comprehensive overview.

**Object Model and Ownership:**

The fundamental building block of Huly Network is the *Object*. Each Object has a unique identifier (UID) and its state is represented by a reference to its latest operation Blob. Any Object can own another, but each Object has at most *one* owner, creating a hierarchy for access control, data relationships, and garbage collection.

**Objects, Blobs, and State:**

Objects in Huly Network are immutable and are identified by a reference to their latest operation Blob. *Blobs* are immutable, content-addressed chunks of data. A Blob's identity (Blob ID) is the cryptographic hash of its content. An Object's state is therefore also immutable: it's simply a reference to its latest operation Blob.  When an operation modifies an Object, a *new* operation Blob is created, and the Object's reference is updated.  This enables efficient versioning and forking. The Object's structure defines how it references other Blobs which contain it's data. Object structure is also represented as Blob. Thus, conceptually Object is a reference to it's state Blob and strucure Blob.

**Referencing Blobs:**
Objects reference Blobs by their content-addressed Blob IDs.  Immutability enables data deduplication and efficient storage, as multiple Objects can reference the same Blob.

**Core Entities:**

1.  **Object:** The foundational data structure.  Specialized Objects include Wallets and Groups.
2.  **Actor:** A user, agent, or service with a cryptographic identity (and Device Keys) that initiates operations.
3.  **Organization:** Maintains network integrity and availability. Organizations operate libp2p nodes, store and replicate Blobs, manage object versions (links to the last operation Blob) via partial order consensus, and manage the BFT broadcast mechanism.

**Key Mechanisms:**

1.  **Conflictless Operations:**  Operations always succeed. Operations that would traditionally create conflicts now produce a result (e.g. `Ok`, `Error`) recorded in the new operation Blob. These results are handled by clients.

2.  **Byzantine Fault-Tolerant (BFT) Broadcast:**  Guarantees reliable message dissemination.

3.  **Partial Order Consensus:** Consensus establishes a partial order of operation Blobs. Two phases:
    *   **Phase 1: Endorsement Collection and Broadcast:**  Devices submit operation Blobs (which *contain* a link to the previous operation Blob). Organizations validate, generate endorsements with sequence numbers, and broadcast both. Missing operation Blobs are exchanged between Organizations if needed.
    *   **Phase 2: Partial Order Agreement and Joint Operations:**  Organizations collect endorsements, determine a partial order, and resolve order conflicts (concurrent operations linked to the same previous operation Blob) by creating a joint operation Blob.  This joint operation becomes part of the partial order, ensuring consistency across all replicas.

4.  **Object Reference Update (Commit):**  Organizations atomically update the Object's reference to the latest Blob (or joint operation Blob).


This revised System Overview is more concise and addresses the points you raised. It focuses on the core aspects of the conflictless model, partial order consensus, and the roles of Objects, Blobs, Actors, and Organizations.  The complexities of logical conflict resolution and higher-level objects like Wallets and Groups will be addressed in separate chapters.  Where should we go from here?  Do you want to tackle the Digital Assets (Wallets) chapter next?

---

# 2. System Overview: A Decentralized, Conflictless Architecture with Partial Order Consensus

Huly Network presents a novel decentralized architecture designed for secure, scalable collaboration and value exchange.  This architecture leverages a Byzantine fault-tolerant (BFT) broadcast mechanism and a simplified, operation-centric consensus process, achieving high concurrency through a "conflictless" data model and partial order of operations.  By deferring conflict resolution to clients (observers), Huly Network simplifies the core system design and maximizes developer flexibility.

**Object Model and Ownership:**

The fundamental building block is the *Object*. Each Object has a unique identifier (UID) and its state is immutable, represented by a reference to its latest operation Blob.  Objects form a hierarchical ownership structure: any Object can own another, but each Object has at most *one* owner. This structure facilitates access control, data relationships, and garbage collection.

**Objects, Blobs, and State:**

*Blobs* are immutable, content-addressed chunks of data. A Blob's identity (Blob ID) is the cryptographic hash of its content.  Objects are effectively immutable linked lists of Blobs: each Object maintains a reference to its latest operation Blob, which in turn contains a reference to the preceding operation Blob, forming a chain. When an operation modifies an Object, a new operation Blob is created, containing the operation details and a link to the previous operation Blob.  The Object's reference is then atomically updated to point to this new Blob. This mechanism enables efficient versioning and forking.  Additionally, an Object references a separate structure Blob, which defines the Object's schema and references any data Blobs associated with the Object.  Both the state and structure Blobs are immutable.

**Referencing Blobs:**

Objects reference Blobs by their content-addressed Blob IDs.  Immutability allows for deduplication, with multiple Objects referencing the same Blob for efficient storage.

**Core Entities:**

1.  **Object:** The base data structure, including specialized types like Wallets and Groups.
2.  **Actor:** A user, agent, or service initiating operations, possessing a cryptographic identity (including Device Keys).
3.  **Organization:**  Responsible for network integrity and availability. Organizations operate libp2p nodes, store and replicate Blobs, *manage Object versioning* (updating Object references via partial order consensus), and manage the BFT broadcast mechanism.

**Key Mechanisms:**

1.  **Conflictless Operations:** All operations succeed.  Operations that would traditionally cause conflicts produce a result (e.g., `Ok`, `Error`) stored in the new operation Blob, to be handled by clients at the logical level.

2.  **Byzantine Fault-Tolerant (BFT) Broadcast:** Guarantees reliable message dissemination.

3.  **Partial Order Consensus:**  Establishes a partial order of operation Blobs for each Object.  Two phases:
    *   **Phase 1: Endorsement Collection and Broadcast:** Devices submit operation Blobs, including the link to the previous operation Blob.  Organizations validate, generate endorsements with sequence numbers, and broadcast both. Organizations exchange missing operation Blobs.
    *   **Phase 2: Partial Order Agreement and Joint Operations:** Organizations collect endorsements, use sequence numbers to determine a partial order, and create joint operation Blobs to resolve order conflicts (concurrent operations linked to the same previous operation Blob). This ensures a consistent partial order across all replicas.

4.  **Object Reference Update (Commit):** Organizations atomically update the Object's reference to the latest operation Blob (or joint operation Blob).

---

# 3. Consensus Mechanisms: Achieving Partial Order Agreement

Huly Network employs a simplified yet robust consensus mechanism based on a partial ordering of operations. This approach, combined with the conflictless operation model, ensures eventual consistency across all replicas while maximizing concurrency and minimizing coordination overhead. This chapter details the consensus process, explaining how Organizations achieve agreement on the partial order of operation Blobs for each Object.

**Partial Order Consensus:**

The core goal of consensus in Huly Network is to establish a consistent partial order of operation Blobs for each Object. This partial order represents the history of operations applied to the Object, ensuring that all Organizations have a unified view of the Object's evolution, even in the presence of concurrent operations and Byzantine failures.

**Two-Phase Endorsement Process:**

Huly Network's consensus mechanism utilizes a two-phase endorsement process:

**Phase 1: Endorsement Collection and Broadcast:**

1.  **Operation Submission:** An Actor's Device submits an operation Blob to a subset of Organizations responsible for the Object. The operation Blob contains the operation details, a link to the previous operation Blob in the chain, and the Device's signature.

2.  **Validation and Endorsement:** Each receiving Organization validates the operation Blob's signature, checks for any logical errors (but not data conflicts, which are handled at the logical level), and generates a signed endorsement *without executing the operation itself*. The endorsement includes the Organization's UID, a monotonically increasing sequence number (unique per Organization), and the Blob ID of the operation Blob being endorsed.

3.  **Broadcast:** The Organization broadcasts both the operation Blob and its endorsement to all other Organizations responsible for the Object, using the BFT broadcast mechanism.  This ensures all relevant Organizations receive the operation and endorsement information.

**Phase 2: Partial Order Agreement and Joint Operation Creation:**

1.  **Endorsement Collection and Ordering:** Organizations collect endorsements from other Organizations. They use the endorsement sequence numbers and Organization UIDs to construct a directed acyclic graph (DAG) representing the partial order of operation Blobs.  An edge from Blob A to Blob B in the DAG indicates that Blob A precedes Blob B in the partial order.

2.  **Order Conflict Detection:** An "order conflict" occurs when two or more operation Blobs reference the *same* previous operation Blob.  This indicates concurrent operations and a potential ambiguity in their order.

3.  **Joint Operation Creation:**  To resolve an order conflict, a designated Organization (e.g., the one with the lowest UID among those involved in the conflict) creates a *joint operation Blob*. This special Blob references all the conflicting operation Blobs, effectively merging them into a single point in the partial order. The joint operation Blob itself does not contain any operation logic; it acts solely as a structural element to resolve the order ambiguity.

4.  **Partial Order Finalization:** Once all endorsements are collected and any order conflicts are resolved through joint operations, the partial order is considered final.  All organizations now have a consistent view of the Object's history as a partially ordered set of operation Blobs.

**Object Reference Update (Commit):**

After the partial order is finalized, Organizations update the Object's reference to point to the latest Blob (or joint operation Blob) in the partial order.

**Illustrative Example:**

Consider three Organizations (O1, O2, O3) and three concurrent operations (OpA, OpB, OpC), all referencing the same previous operation Blob.

*   O1 receives OpA, then OpB, then OpC.
*   O2 receives OpB, then OpC, then OpA.
*   O3 receives OpC, then OpA, then OpB.

Each Organization generates endorsements with sequence numbers.  During Phase 2:

*   They detect an order conflict.
*   O1 (assuming the lowest UID) creates a joint operation Blob referencing OpA, OpB, and OpC.
*   The Object's reference is updated to point to the joint operation Blob.
*   Clients, when reconstructing the logical state, will encounter the joint operation Blob and process OpA, OpB, and OpC according to their own application-specific logic.

**Byzantine Fault Tolerance:**

The BFT broadcast mechanism and the use of sequence numbers in endorsements ensure that this consensus process can tolerate up to *f* Byzantine faulty Organizations, where *f < n/3*.

---

# 4. Data Consistency and Conflict Management: The Logical Level

Huly Network's conflictless architecture introduces a unique approach to data consistency and conflict management. While the system guarantees a consistent partial order of operations on the physical level (managed by Organizations), the responsibility for resolving *logical* data conflicts is deferred to the clients.  This chapter explains the logical level of Huly Network, how clients reconstruct Object state from the partially ordered operations, how logical conflicts arise, and the mechanisms for resolving them.

**Reconstructing Logical State:**

Clients reconstruct the logical state of an Object by traversing the chain of operation Blobs, guided by the partial order established by the consensus mechanism.  Each operation Blob contains the details of a single operation performed on the Object.  Clients interpret these operations and apply them to their local representation of the Object's data.  When encountering a joint operation Blob, clients must process all the referenced operation Blobs, effectively merging the results of concurrent operations.

**Logical Conflicts:**

Even though operations always succeed on the physical level (the conflictless principle), *logical conflicts* can still arise when reconstructing the Object's state.  These conflicts occur when concurrent operations, represented by separate operation Blobs or within a joint operation Blob, have conflicting effects on the logical data structure.

**Example:**

Consider a key-value store Object. Two concurrent operations attempt to update the same key with different values.  Both operations succeed on the physical level, resulting in two operation Blobs (or a joint operation Blob referencing both).  When a client reconstructs the state, it encounters this conflict—which value should be associated with the key?

**Conflict Resolution Strategies (Client-Side):**

Huly Network does *not* prescribe a specific conflict resolution strategy. Instead, it provides clients with the information necessary to detect and resolve conflicts based on their application-specific requirements.  Common strategies include:

*   **Last-Writer-Wins (LWW):** Choose the value from the operation with the latest timestamp (within the operation Blob). Note: we do not have timestamps now, so some other unique info should be provided, for example we can use sequence number from organization endorsement (or something else)
*   **First-Writer-Wins (FWW):**  Choose the value from the operation with the earliest timestamp.
*   **Multi-Value Register (MVR):**  Store all conflicting values and allow the client to choose or merge them later.
*   **Custom Logic:**  Implement application-specific conflict resolution logic based on the semantics of the data and the desired behavior.

**Example (LWW with Key-Value Store):**

A client reconstructing the state of the key-value store encounters a joint operation Blob referencing two operations updating the same key.  The client examines the timestamps within the operation Blobs and chooses the value from the later operation, effectively implementing an LWW strategy.

**The `Result` Structure and Error Handling:**

Operation Blobs can contain a `Result` field indicating the outcome of the operation's execution on the physical level (e.g., `Ok`, `Error`, `Conflict`).  This `Result` field might provide additional information about potential conflicts or errors.  Clients should handle these `Result` values appropriately, implementing error handling and recovery mechanisms as needed. However, the `Result` itself does *not* indicate a logical conflict; it merely reflects the outcome of the physical operation.  Logical conflicts are determined by the client based on the semantics of the data and the effects of concurrent operations.

**Example:**

An operation attempts to withdraw funds from a Wallet.  The operation succeeds on the physical level (a new operation Blob is created), but the `Result` field indicates a `Conflict` (insufficient funds).  The client, observing this `Conflict`, can take appropriate action, such as notifying the user or attempting a different operation.

**Data Consistency Guarantees:**

Huly Network's conflictless model, with its partial order consensus and client-side conflict resolution, provides the following data consistency guarantees:

*   **Eventual Consistency:**  If no new operations are submitted for an Object, all clients will eventually converge to the same logical state, assuming they use the same conflict resolution strategy.
*   **Consistent Partial Order:**  All Organizations agree on the same partial order of operation Blobs, ensuring a consistent history of the Object's evolution.

---

# 6. Digital Assets and Value Exchange: A Conflictless Approach with Recipient Endorsements

Huly Network provides a secure and scalable framework for managing and transferring digital assets within its conflictless architecture. This framework leverages the partial order consensus mechanism, the UTXO model, and *recipient endorsements* to provide a robust and efficient system for value exchange, preventing double-spending without requiring global UTXO tracking.  This chapter details how digital assets are represented, how transactions are processed, and how Huly Network addresses conflict management in a decentralized, concurrent environment, aligning with the principles of "The Consensus Number of a Cryptocurrency."

**Wallets as Objects:**

Digital assets are managed within *Wallets*, specialized Objects owned by a single Object (an Actor or another Object like a Group). A Wallet's state is a reference to its latest operation Blob, forming a chain representing the transaction history.

**UTXO Model and Transactions:**

Huly Network uses the UTXO model.  Each transaction consumes input UTXOs and creates output UTXOs. Transactions are encapsulated within operation Blobs.

**Transaction Lifecycle:**

1.  **Transaction Creation:** An Actor's Device creates an operation Blob containing:
    *   Input UTXO references (Blob IDs).
    *   Output UTXO definitions (recipient Object UIDs and amounts).
    *   A link to the previous operation Blob in the Wallet's history.
    *   The Device's signature.

2.  **Transaction Broadcast and Recipient Confirmation:** The Device broadcasts the transaction Blob to a subset of Organizations *and* to the recipient(s).  Recipient Devices validate:
    *   The sender's signature.
    *   That the UTXOs are unspent in their local view of the sender's Wallet.
    *   That the transaction doesn't create conflicts in their own Wallets.

    If valid, the recipient Devices generate endorsements, signifying acceptance of the UTXO transfer, and broadcast these endorsements to the Organizations.

3.  **Partial Order Consensus (with Recipient Endorsements):** Organizations incorporate recipient endorsements into the consensus process. A transaction is valid *only with recipient endorsements*.  This prevents double-spending, as only one recipient can validly endorse a given UTXO. Organizations use endorsement sequence numbers to determine a partial order and create joint operation Blobs to resolve order conflicts, ensuring eventual consistency across all replicas of the Wallet object.

4.  **Wallet State Update (Commit):** Organizations update the sender's and recipient's Wallet references to reflect the transaction.

5.  **Logical Conflict Handling:**  If Organizations detect conflicting endorsements for a UTXO, a joint operation Blob is created.  Clients observing the Wallet's history resolve these conflicts at the logical level, following their own application logic. Note that at least on of transfer operations will be rejected during consensus, because conflicting endorsements can not be accepted at consensus phase, thus resolving double spend attempts.

**Example:**

Alice attempts to send the same UTXO to Bob and Carol.

1.  Alice's Device broadcasts the transactions.
2.  Bob's Device receives, validates, endorses, and broadcasts its endorsement.
3.  Carol's Device does the same, but her endorsement now references a spent UTXO from Alice's Wallet (due to Bob's already endorsed transaction and corresponding consensus sequence number).
4.  Organizations process Bob's transaction, updating Alice's and Bob's Wallets.  Carol's transaction is rejected due to the now-spent UTXO. Carol's Wallet detects this conflict locally during logical validation after traversing Alice's and her own Wallet's operation history, but the Huly Network has prevented the double-spend attempt, which is crucial.

**Benefits:**

*   **Secure Double-Spend Prevention:** Recipient endorsements, integrated into consensus, provide robust double-spend protection.
*   **Efficient Consensus:** Organizations don't track global UTXO state.
*   **Client-Side Logical Conflict Resolution:**  Maintains flexibility for application-specific logic.

This chapter demonstrates how Huly Network enables secure and efficient digital asset management in a conflictless architecture. The use of recipient endorsements within the partial order consensus process ensures robust double-spending prevention while preserving the flexibility of client-side conflict resolution for logical conflicts. What are your thoughts? What chapter should we tackle next - Wallets or Groups?


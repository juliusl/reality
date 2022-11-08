use crate::wire::{Frame, Protocol};
use specs::{Entities, Entity, Join, Write, WriteStorage};
use std::{collections::HashMap, io::Cursor};
use tokio::{io::AsyncReadExt, task::JoinSet};
use tracing::{event, Level};

use super::{Journal, Transaction};

pub type ProtocolEntry = tokio::sync::watch::Sender<Protocol>;

/// Frame database,
///
pub struct Database<'a> {
    /// Protocols being managed by the database,
    ///
    protocols: Write<'a, Option<HashMap<Entity, ProtocolEntry>>>,
    /// Entities storage,
    ///
    entities: Entities<'a>,
    /// Transactions storage,
    ///
    transactions: WriteStorage<'a, Transaction>,
    /// Journal storage,
    ///
    journal: WriteStorage<'a, Journal>,
}

impl<'a> Database<'a> {
    /// Handles open transactions,
    ///
    pub async fn handle_transactions(&mut self) {
        let transactions = (&self.entities, self.transactions.drain())
            .join()
            .collect::<Vec<_>>();

        let mut joinset = JoinSet::<(Entity, Transaction, ProtocolEntry)>::new();

        if let Some(mut protocols) = self.protocols.take() {
            let protocols = &mut protocols;
            for (entity, transaction) in transactions {
                if let Some(protocol_entry) = protocols.remove(&entity) {
                    joinset.spawn(async move {
                        let protocol_entry = protocol_entry;
                        let mut transaction = transaction;
                        let mut frame = [0; 64];
                        match transaction.frames.1.read_exact(&mut frame).await {
                            Ok(read) => {
                                assert_eq!(read, 64);
                            }
                            Err(err) => {
                                event!(Level::ERROR, "Error reading transaction, {err}");
                            }
                        }
                        let frame = Frame::from(frame);
                        let mut control_frame = [0; 64];
                        match transaction.control.1.read_exact(&mut control_frame).await {
                            Ok(read) => {
                                assert_eq!(read, 64);
                            }
                            Err(err) => {
                                event!(Level::ERROR, "Error reading transaction, {err}");
                            }
                        }
                        let control_frame = Frame::from(control_frame);
                        let mut buf = bytes::BytesMut::new();
                        match transaction.blob.1.read_buf(&mut buf).await {
                            Ok(_) => {}
                            Err(err) => {
                                event!(Level::ERROR, "Could read from blob, {err}")
                            }
                        }
                        let buf = buf.freeze();
                        let mut buf = Cursor::new(buf);
                        protocol_entry.send_if_modified(|protocol| {
                            protocol.encoder_by_id(&transaction.resource_id, |_, encoder| {
                                encoder.frames.push(frame);
                                match std::io::copy(&mut buf, &mut encoder.blob_device) {
                                    Ok(_) => {}
                                    Err(err) => {
                                        event!(Level::ERROR, "Error copying blob data, {err}")
                                    }
                                }
                            });
                            false
                        });

                        if !transaction.control_device.accept(control_frame) {
                            event!(Level::ERROR, "Received an invalid control frame");
                        }
                        (entity, transaction, protocol_entry)
                    });
                }
            }
        }

        let mut next_state = HashMap::new();
        while let Some(result) = joinset.join_next().await {
            match result {
                Ok((entity, transaction, entry)) if transaction.commit.is_some() => {
                    self.transactions
                        .insert(entity, transaction)
                        .expect("should be able to insert");
                    next_state.insert(entity, entry);
                }
                Ok((entity, transaction, entry)) if transaction.commit.is_none() => {
                    entry.send_if_modified(|protocol| {
                        protocol.encoder_by_id(&transaction.resource_id, |_, encoder| {
                            encoder.interner = transaction.control_device.into();
                        });
                        true
                    });
                    next_state.insert(entity, entry);
                    self.journal
                        .insert(entity, Journal::default())
                        .expect("should be able to insert");
                }
                Err(err) => {
                    event!(Level::ERROR, "Error getting next result, {err}");
                }
                _ => {}
            }
        }

        *self.protocols = Some(next_state);
    }
}

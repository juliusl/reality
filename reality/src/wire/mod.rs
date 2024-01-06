mod frame;
mod op;
mod packet;
mod routes;
mod server;

pub mod prelude {
    pub use super::frame::Frame;
    pub use super::frame::FrameListener;
    pub use super::frame::FrameUpdates;
    pub use super::frame::ToFrame;
    pub use super::op::Code;
    pub use super::op::Op;
    pub use super::packet::FieldPacket;
    pub use super::packet::FieldPacketType;
    pub use super::routes::FieldIndex;
    pub use super::routes::FieldKey;
    pub use super::routes::PacketRouter;
    pub use super::routes::PacketRoutes;
    pub use super::server::enable_virtual_dependencies;
    pub use super::server::FieldRefController;
    pub use super::server::WireClient;
    pub use super::server::WireServer;
}

#[allow(unused_imports)]
mod test {
    use crate::prelude::*;
    use anyhow::anyhow;
    use async_stream::stream;
    use async_trait::async_trait;
    use futures_util::pin_mut;
    use futures_util::StreamExt;
    use serde::Serialize;
    use std::time::Duration;
    use tokio::join;
    use tokio::time::Instant;

    #[derive(Reality, Clone, Serialize, Default)]
    #[reality(call=test_noop, plugin)]
    struct Test {
        #[reality(derive_fromstr)]
        name: String,
        other: String,
    }

    async fn test_noop(_tc: &mut ThunkContext) -> anyhow::Result<()> {
        Ok(())
    }

    #[test]
    fn test_packet() {
        let packet = crate::FieldPacket::new_data(String::from("Hello World"));
        let packet = packet.into_box::<String>();
        let packet_data = packet.expect("should be able to convert");
        let packet_data = packet_data.as_str();
        assert_eq!("Hello World", packet_data);

        let packet = crate::FieldPacket::new_data(String::from("Hello World"));
        // let packet = packet.into_box::<Vec<u8>>();
        // assert!(packet.is_none());
    }

    #[tokio::test]
    async fn test_frame_listener() {
        let mut _frame_listener = FrameListener::with_buffer::<1>(Test {
            name: String::from("cool name"),
            other: String::from("hello other world"),
        });

        let tx = _frame_listener.routes();

        let field_ref = tx
            .borrow()
            .virtual_ref()
            .name
            .clone()
            .start_tx()
            .next(|f| {
                assert!(f.edit_value(|_, v| {
                    *v = String::from("really cool name");
                    true
                }));

                Ok(f)
            })
            .finish()
            .unwrap();

        let packet = field_ref.encode();

        let permit = _frame_listener.new_tx().await.unwrap();
        permit.send(vec![packet]);

        let next = _frame_listener.listen().await.unwrap();
        eprintln!("{:#?}", next);
        ()
    }

    #[tokio::test]
    async fn test_frame_router() {
        // Create a new node
        let node = Shared::default().into_thread_safe_with(tokio::runtime::Handle::current());

        // Simulate a thunk context being used
        let mut tc: ThunkContext = node.into();

        // Create a new wire server/client for this plugin Test
        let server = WireServer::<Test>::new(&mut tc).await.unwrap();

        tokio::spawn(server.clone().start());

        let client = server.clone().new_client();

        client
            .try_borrow_modify(|t| {
                t.virtual_ref().name.edit_value(|_, n| {
                    *n = String::from("hello world cool test 2");
                    true
                });

                Ok(t.virtual_ref().name.encode())
            })
            .unwrap();

        // Simulate a concurrent process starting up subsequently
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Simulate receiving changes on thunk execution
        let test = Remote.create::<Test>(&mut tc).await;

        test.to_virtual().name.view_value(|v| {
            assert_eq!("hello world cool test 2", v);
        });

        client
            .try_borrow_modify(|t| {
                t.virtual_ref().name.edit_value(|_, n| {
                    *n = String::from("hello world cool test 3");
                    true
                });
                Ok(t.virtual_ref().name.encode())
            })
            .unwrap();

        tokio::time::sleep(Duration::from_millis(500)).await;

        // Simulate receiving changes on thunk execution
        let test = Remote.create::<Test>(&mut tc).await;

        test.to_virtual().name.view_value(|v| {
            assert_eq!("hello world cool test 3", v);
        });

        ()
    }
}

use fuel_core::{
    database::Database,
    service::{
        Config,
        FuelService,
    },
};
use fuel_core_client::client::{
    FuelClient,
    PageDirection,
    PaginationRequest,
};
use fuel_core_poa::Trigger;
use fuel_core_types::{
    fuel_asm::*,
    fuel_crypto::SecretKey,
    fuel_tx::{
        Finalizable,
        TransactionBuilder,
    },
    secrecy::Secret,
};
use rand::{
    rngs::StdRng,
    SeedableRng,
};

#[tokio::test(start_paused = true)]
async fn poa_instant_trigger_is_produces_instantly() {
    let mut rng = StdRng::seed_from_u64(10);

    let db = Database::default();
    let mut config = Config::local_node();
    config.consensus_key = Some(Secret::new(SecretKey::random(&mut rng).into()));
    config.block_production = Trigger::Instant;

    let srv = FuelService::from_database(db.clone(), config)
        .await
        .unwrap();

    let client = FuelClient::from(srv.bound_address);

    for i in 0..10usize {
        let mut tx = TransactionBuilder::script(
            [op::movi(0x10, i.try_into().unwrap())]
                .into_iter()
                .collect(),
            vec![],
        );
        let _tx_id = client.submit(&tx.finalize().into()).await.unwrap();
        let count = client
            .blocks(PaginationRequest {
                cursor: None,
                results: 1024,
                direction: PageDirection::Forward,
            })
            .await
            .expect("blocks request failed")
            .results
            .len();

        let block_number = i + 1;
        assert_eq!(count, block_number + 1 /* genesis block */);
    }
}

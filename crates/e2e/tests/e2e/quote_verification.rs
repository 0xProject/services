use {
    bigdecimal::{BigDecimal, Zero},
    e2e::{setup::*, tx},
    ethcontract::{H160, U256},
    ethrpc::Web3,
    model::{
        interaction::InteractionData,
        order::{BuyTokenDestination, OrderKind, SellTokenSource},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
    },
    number::nonzero::U256 as NonZeroU256,
    serde_json::json,
    shared::{
        addr,
        price_estimation::{
            Estimate,
            Verification,
            trade_verifier::{
                PriceQuery,
                TradeVerifier,
                TradeVerifying,
                balance_overrides::BalanceOverrides,
            },
        },
        trade_finding::{Interaction, LegacyTrade, QuoteExecution, TradeKind},
    },
    std::{str::FromStr, sync::Arc},
};

#[tokio::test]
#[ignore]
async fn local_node_standard_verified_quote() {
    run_test(standard_verified_quote).await;
}

#[tokio::test]
#[ignore]
async fn forked_node_bypass_verification_for_rfq_quotes() {
    run_forked_test_with_block_number(
        test_bypass_verification_for_rfq_quotes,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK_MAINNET,
    )
    .await;
}

#[tokio::test]
#[ignore]
async fn local_node_verified_quote_eth_balance() {
    run_test(verified_quote_eth_balance).await;
}

#[tokio::test]
#[ignore]
async fn local_node_verified_quote_for_settlement_contract() {
    run_test(verified_quote_for_settlement_contract).await;
}

#[tokio::test]
#[ignore]
async fn local_node_verified_quote_with_simulated_balance() {
    run_test(verified_quote_with_simulated_balance).await;
}

#[tokio::test]
#[ignore]
async fn forked_node_mainnet_usdt_quote() {
    run_forked_test_with_block_number(
        usdt_quote_verification,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        21422760,
    )
    .await;
}

/// Verified quotes work as expected.
async fn standard_verified_quote(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(10)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    token.mint(trader.address(), to_wei(1)).await;
    tx!(
        trader.account(),
        token.approve(onchain.contracts().allowance, to_wei(1))
    );

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // quote where the trader has sufficient balance and an approval set.
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: trader.address(),
            sell_token: token.address(),
            buy_token: onchain.contracts().weth.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: to_wei(1).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);
}

/// The block number from which we will fetch state for the forked tests.
const FORK_BLOCK_MAINNET: u64 = 19796077;

/// Tests that quotes requesting `tx_origin: 0x0000` bypass the verification
/// because those are currently used by some solvers to provide market maker
/// integrations. Based on an RFQ quote we saw on prod:
/// https://www.tdly.co/shared/simulation/7402de5e-e524-4e24-9af8-50d0a38c105b
async fn test_bypass_verification_for_rfq_quotes(web3: Web3) {
    let url = std::env::var("FORK_URL_MAINNET")
        .expect("FORK_URL_MAINNET must be set to run forked tests")
        .parse()
        .unwrap();
    let block_stream =
        ethrpc::block_stream::current_block_stream(url, std::time::Duration::from_millis(1_000))
            .await
            .unwrap();
    let onchain = OnchainComponents::deployed(web3.clone()).await;

    let verifier = TradeVerifier::new(
        web3.clone(),
        Arc::new(web3.clone()),
        Arc::new(web3.clone()),
        Arc::new(BalanceOverrides::default()),
        block_stream,
        onchain.contracts().gp_settlement.address(),
        onchain.contracts().weth.address(),
        BigDecimal::zero(),
    )
    .await
    .unwrap();

    let verify_trade = |tx_origin| {
        let verifier = verifier.clone();
        async move {
            verifier
                .verify(
                    &PriceQuery {
                        sell_token: H160::from_str("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599")
                            .unwrap(),
                        buy_token: H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
                            .unwrap(),
                        kind: OrderKind::Sell,
                        in_amount: NonZeroU256::new(12.into()).unwrap(),
                    },
                    &Verification {
                        from: H160::from_str("0x73688c2b34bf6c09c125fed02fe92d17a94b897a").unwrap(),
                        receiver: H160::from_str("0x73688c2b34bf6c09c125fed02fe92d17a94b897a")
                            .unwrap(),
                        pre_interactions: vec![],
                        post_interactions: vec![],
                        sell_token_source: SellTokenSource::Erc20,
                        buy_token_destination: BuyTokenDestination::Erc20,
                    },
                    TradeKind::Legacy(LegacyTrade {
                        out_amount: 16380122291179526144u128.into(),
                        gas_estimate: Some(225000),
                        interactions: vec![Interaction {
                            target: H160::from_str("0xdef1c0ded9bec7f1a1670819833240f027b25eff")
                                .unwrap(),
                            data: hex::decode("aa77476c000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000002260fac5e5542a773aa44fbcfedf7c193bc2c599000000000000000000000000000000000000000000000000e357b42c3a9d8ccf0000000000000000000000000000000000000000000000000000000004d0e79e000000000000000000000000a69babef1ca67a37ffaf7a485dfff3382056e78c0000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab41000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000066360af101ffffffffffffffffffffffffffffffffffffff0f3f47f166360a8d0000003f0000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000001c66b3383f287dd9c85ad90e7c5a576ea4ba1bdf5a001d794a9afa379e6b2517b47e487a1aef32e75af432cbdbd301ada42754eaeac21ec4ca744afd92732f47540000000000000000000000000000000000000000000000000000000004d0c80f").unwrap(),
                            value: 0.into(),
                        }],
                        solver: H160::from_str("0xe3067c7c27c1038de4e8ad95a83b927d23dfbd99")
                            .unwrap(),
                        tx_origin,
                    }),
                )
                .await
        }
    };

    let verified_quote = Estimate {
        out_amount: 16380122291179526144u128.into(),
        gas: 225000,
        solver: H160::from_str("0xe3067c7c27c1038de4e8ad95a83b927d23dfbd99").unwrap(),
        verified: true,
        execution: QuoteExecution {
            interactions: vec![InteractionData {
                target: H160::from_str("0xdef1c0ded9bec7f1a1670819833240f027b25eff").unwrap(), 
                value: 0.into(),
                call_data: hex::decode("aa77476c000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000002260fac5e5542a773aa44fbcfedf7c193bc2c599000000000000000000000000000000000000000000000000e357b42c3a9d8ccf0000000000000000000000000000000000000000000000000000000004d0e79e000000000000000000000000a69babef1ca67a37ffaf7a485dfff3382056e78c0000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab41000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000066360af101ffffffffffffffffffffffffffffffffffffff0f3f47f166360a8d0000003f0000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000001c66b3383f287dd9c85ad90e7c5a576ea4ba1bdf5a001d794a9afa379e6b2517b47e487a1aef32e75af432cbdbd301ada42754eaeac21ec4ca744afd92732f47540000000000000000000000000000000000000000000000000000000004d0c80f").unwrap() 
            }],
            pre_interactions: vec![],
            jit_orders: vec![],
        },
    };

    // `tx_origin: 0x0000` is currently used to bypass quote verification due to an
    // implementation detail of zeroex RFQ orders.
    // TODO: remove with #2693
    let verification = verify_trade(Some(H160::zero())).await;
    assert_eq!(&verification.unwrap(), &verified_quote);

    // Trades using any other `tx_origin` can not bypass the verification.
    let verification = verify_trade(None).await;
    assert_eq!(
        verification.unwrap(),
        Estimate {
            verified: false,
            ..verified_quote
        }
    );
}

/// Verified quotes work as for WETH trades without wrapping or approvals.
async fn verified_quote_eth_balance(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(10)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;
    let weth = &onchain.contracts().weth;

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // quote where the trader has no WETH balances or approval set, but
    // sufficient ETH for the trade
    assert_eq!(
        (
            weth.balance_of(trader.address()).call().await.unwrap(),
            weth.allowance(trader.address(), onchain.contracts().allowance)
                .call()
                .await
                .unwrap(),
        ),
        (U256::zero(), U256::zero()),
    );
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: trader.address(),
            sell_token: weth.address(),
            buy_token: token.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: to_wei(1).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);
}

/// Test that asserts that we can verify quotes where the settlement contract is
/// the trader or receiver.
async fn verified_quote_for_settlement_contract(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(10)).await;
    let [trader] = onchain.make_accounts(to_wei(3)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Send 3 ETH to the settlement contract so we can get verified quotes for
    // selling WETH.
    onchain
        .send_wei(onchain.contracts().gp_settlement.address(), to_wei(3))
        .await;

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver.clone()).await;

    let request = OrderQuoteRequest {
        sell_token: onchain.contracts().weth.address(),
        buy_token: token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: to_wei(3).try_into().unwrap(),
            },
        },
        ..Default::default()
    };

    // quote where settlement contract is trader and implicit receiver
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: onchain.contracts().gp_settlement.address(),
            receiver: None,
            ..request.clone()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // quote where settlement contract is trader and explicit receiver
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: onchain.contracts().gp_settlement.address(),
            receiver: Some(onchain.contracts().gp_settlement.address()),
            ..request.clone()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // quote where settlement contract is trader and not the receiver
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: onchain.contracts().gp_settlement.address(),
            receiver: Some(trader.address()),
            ..request.clone()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // quote where a random trader sends funds to the settlement contract
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: trader.address(),
            receiver: Some(onchain.contracts().gp_settlement.address()),
            ..request.clone()
        })
        .await
        .unwrap();
    assert!(response.verified);
}

/// Test that asserts that we can verify quotes for traders with simulated
/// balances.
async fn verified_quote_with_simulated_balance(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(10)).await;
    let [trader] = onchain.make_accounts(to_wei(0)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;
    let weth = &onchain.contracts().weth;

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                api: vec![
                    // The OpenZeppelin `ERC20Mintable` token uses a mapping in
                    // the first (0'th) storage slot for balances.
                    format!("--quote-token-balance-overrides={:?}@0", token.address()),
                    // We don't configure the WETH token and instead rely on
                    // auto-detection for balance overrides.
                    "--quote-autodetect-token-balance-overrides=true".to_string(),
                ],
                ..Default::default()
            },
            solver,
        )
        .await;

    // quote where the trader has no balances or approval set from TOKEN->WETH
    assert_eq!(
        (
            token.balance_of(trader.address()).call().await.unwrap(),
            token
                .allowance(trader.address(), onchain.contracts().allowance)
                .call()
                .await
                .unwrap(),
        ),
        (U256::zero(), U256::zero()),
    );
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: trader.address(),
            sell_token: token.address(),
            buy_token: weth.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: to_wei(1).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // quote where the trader has no balances or approval set from WETH->TOKEN
    assert_eq!(
        (
            onchain
                .web3()
                .eth()
                .balance(trader.address(), None)
                .await
                .unwrap(),
            weth.balance_of(trader.address()).call().await.unwrap(),
            weth.allowance(trader.address(), onchain.contracts().allowance)
                .call()
                .await
                .unwrap(),
        ),
        (U256::zero(), U256::zero(), U256::zero()),
    );
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: trader.address(),
            sell_token: weth.address(),
            buy_token: token.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: to_wei(1).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // with balance overrides we can even verify quotes for the 0 address
    // which is used when no wallet is connected in the frontend
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: H160::zero(),
            sell_token: weth.address(),
            buy_token: token.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: to_wei(1).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // Previously quote verification did not set up the trade correctly
    // if the user provided pre-interactions. This works now.
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: H160::zero(),
            sell_token: weth.address(),
            buy_token: token.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: to_wei(1).try_into().unwrap(),
                },
            },
            app_data: model::order::OrderCreationAppData::Full {
                full: json!({
                    "metadata": {
                        "hooks": {
                            "pre": [
                                {
                                    "target": "0x0000000000000000000000000000000000000000",
                                    "callData": "0x",
                                    "gasLimit": "0"
                                }
                            ]
                        }
                    }
                })
                .to_string(),
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);
}

/// Ensures that quotes can even be verified with tokens like `USDT`
/// which are not completely ERC20 compliant.
async fn usdt_quote_verification(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;

    let [solver] = onchain.make_solvers_forked(to_wei(1)).await;

    let usdc = addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let usdt = addr!("dac17f958d2ee523a2206206994597c13d831ec7");

    // Place Orders
    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                api: vec!["--quote-autodetect-token-balance-overrides=true".to_string()],
                ..Default::default()
            },
            solver,
        )
        .await;

    let quote = services
        .submit_quote(&OrderQuoteRequest {
            sell_token: usdt,
            buy_token: usdc,
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: to_wei_with_exp(1000, 18).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(quote.verified);
}

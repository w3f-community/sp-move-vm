script {
    use 0x01::Account;
    use 0x01::Dfinance;

    fun load_coin(s: &signer, expected_values: u128) {
        Account::store_balance<Dfinance::BTC>(Dfinance::mint<Dfinance::BTC>(expected_values), s);
    }
}
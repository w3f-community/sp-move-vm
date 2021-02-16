address 0x01 {
    module BalanceTest {
        use 0x01::Dfinance;
        use 0x01::Call;
        //use 0x01::Account;

        resource struct Lock1<Coin: resource> {
            coins: Dfinance::T<Coin>
        }

        resource struct Lock2 {
            coins: Dfinance::T<Dfinance::BTC>
        }

        resource struct InnerGeneric<TP: resource> {
            tp: TP
        }

        resource struct Lock3<Coin: resource> {
            foo: u8,
            inner: InnerGeneric<Dfinance::T<Coin>>,
            bar: u8,
            inner2: InnerGeneric<Dfinance::T<Coin>>,
        }

        resource struct Lock4<DFI: resource, DFI2: resource> {
            foo: u8,
            inner: InnerGeneric<DFI>,
            bar: u8,
            inner2: InnerGeneric<DFI>,
        }

        resource struct Lock5<DFI: resource> {
            foo: u8,
            inner: InnerGeneric<Dfinance::T<Dfinance::BTC>>,
            bar: u8,
            inner2: InnerGeneric<Dfinance::T<Dfinance::USD>>,
        }

        resource struct Call__ {
            foo: Call::Foo
        }
    }
}
address 0x01 {
module Signer {
    native public fun borrow_address(s: &signer): &address;

    public fun address_of(s: &signer): address {
        *borrow_address(s)
    }

    spec module {
        native define get_address(account: signer): address;
    }
}

module Account {
    use 0x01::Dfinance;
    use 0x01::Signer;

    resource struct Balance<Token: resource> {
        coin: Dfinance::T<Token>
    }

    public fun has_balance<Token: resource>(payee: address): bool {
        exists<Balance<Token>>(payee)
    }

    public fun withdraw_from_sender<Token: resource>(account: &signer, amount: u128): Dfinance::T<Token> acquires Balance {
        let balance = borrow_global_mut<Balance<Token>>(Signer::address_of(account));
        Dfinance::withdraw(&mut balance.coin, amount)
    }

    public fun store_balance<Token: resource>(coin: Dfinance::T<Token>, addr: &signer) {
        move_to<Balance<Token>>(addr, Balance<Token> { coin: coin });
    }

    public fun deposit_to_sender<Token: resource>(
        account: &signer,
        to_deposit: Dfinance::T<Token>
    ) acquires Balance {
        let amount = Dfinance::value(&to_deposit);
        assert(amount > 0, 1);
        let payee_balance = borrow_global_mut<Balance<Token>>(Signer::address_of(account));
        Dfinance::deposit(&mut payee_balance.coin, to_deposit);
    }
}

module Dfinance {
    resource struct BTC {}

    resource struct HTS {}

    resource struct USD {}

    resource struct T<Coin: resource> {
        value: u128
    }

    public fun mint<Coin: resource>(value: u128): T<Coin> {
        T { value }
    }

    public fun withdraw<Coin: resource>(coin: &mut T<Coin>, amount: u128): T<Coin> {
        assert(coin.value >= amount, 0);
        coin.value = coin.value - amount;
        T { value: amount }
    }

    public fun value<Coin: resource>(coin: &T<Coin>): u128 {
        coin.value
    }

    public fun deposit<Coin: resource>(coin: &mut T<Coin>, check: T<Coin>) {
        let T { value } = check; // destroy check
        coin.value = coin.value + value;
    }
}
}

module Call {
    use 0x01::Dfinance;

    resource struct Test<T> {
        t: T,
        cal: Dfinance::T<Dfinance::USD>,
        foo: Foo,
    }

    resource struct Foo {
        foo2: address,
        bar: Bar,
        foo3: address,
        bar2: Bar,
        cal: Dfinance::T<Dfinance::HTS>,
    }

    resource struct Bar {
        bar: u8,
        foo: u64,
        foo1: Dfinance::T<Dfinance::USD>,
        foo2: address
    }

    resource struct Call<Coin, T: resource, D> {
        cal: Dfinance::T<T>
    }

    public fun create_call<Coin, T: resource, D>(cal: Dfinance::T<T>): Call<Coin, T, D> {
        Call<Coin, T, D> { cal: cal }
    }

    public fun store_call<Coin, T: resource, D>(call: Call<Coin, T, D>, addr: &signer) {
        move_to<Call<Coin, T, D>>(addr, call);
    }

    public fun store_test<Coin: resource>(addr: &signer) {
        let test = Test {
            t: Dfinance::mint<Coin>(100),
            cal: Dfinance::mint<Dfinance::USD>(100),
            foo: Foo {
                foo2: 0x01,
                bar: Bar {
                    bar: 0,
                    foo: 0,
                    foo1: Dfinance::mint<Dfinance::USD>(100),
                    foo2: 0x1
                },
                foo3: 0x002,
                bar2: Bar {
                    bar: 0,
                    foo: 0,
                    foo1: Dfinance::mint<Dfinance::USD>(100),
                    foo2: 0x3
                },
                cal: Dfinance::mint<Dfinance::HTS>(100),
            }
        };

        move_to<Test<Dfinance::T<Coin>>>(addr, test);
    }
}

script {
    use 0x01::Dfinance;
    use 0x01::Account;
    use 0x01::Call;

    fun load_modify_and_store_balance(addr: &signer) {
        Account::deposit_to_sender<Dfinance::BTC>(addr, Dfinance::mint<Dfinance::BTC>(1000));
        Call::store_call<Dfinance::BTC, Dfinance::HTS, u64>(Call::create_call<Dfinance::BTC, Dfinance::HTS, u64>(Dfinance::mint<Dfinance::HTS>(1313)), addr);
        Call::store_test<Dfinance::HTS>(addr);
    }
}

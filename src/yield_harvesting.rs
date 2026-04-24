#[test]
fn test_harvest_yield_pro_rata() {
    let env = Env::default();
    let user1 = Address::random(&env);
    let user2 = Address::random(&env);

    env.storage().set(&user1, &100);
    env.storage().set(&user2, &200);
    env.storage().set(&Symbol::short("members"), &vec![user1.clone(), user2.clone()]);

    YieldContract::harvest_yield(env.clone(), 90, 0);

    let bal1: i128 = env.storage().get(&user1).unwrap();
    let bal2: i128 = env.storage().get(&user2).unwrap();

    assert_eq!(bal1, 100 + 30); // 1/3 of yield
    assert_eq!(bal2, 200 + 60); // 2/3 of yield
}

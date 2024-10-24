use z3::{
    ast::{Ast, Int},
    Config, Context, Solver,
};

#[test]
fn z3_works() {
    let config = Config::new();
    let ctx = Context::new(&config);
    let solver = Solver::new(&ctx);

    let x = Int::new_const(&ctx, "x");
    let y = Int::new_const(&ctx, "y");

    let _0 = Int::from_i64(&ctx, 0);
    let _5 = Int::from_i64(&ctx, 5);
    let _15 = Int::from_i64(&ctx, 15);

    solver.assert(&(&x + &_5 * &y).le(&_15));
    solver.assert(&x.ge(&_0));
    solver.assert(&x.le(&_5));
    solver.assert(&y.ge(&_0));

    solver.assert(&(&x + &y)._eq(&_5));

    match solver.check() {
        z3::SatResult::Sat => {
            let model = solver.get_model();
            println!("Model: {:?}", model);
        }
        z3::SatResult::Unsat => {
            println!("unsat");
        }
        z3::SatResult::Unknown => {
            println!("unknown");
        }
    }
}

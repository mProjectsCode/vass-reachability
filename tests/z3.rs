use z3::{
    Config, Context, SatResult, Solver,
    ast::{Ast, Int},
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
    let _17 = Int::from_i64(&ctx, 17);

    // both x and y are positive
    solver.assert(&x.ge(&_0));
    solver.assert(&y.ge(&_0));

    // x + 5 * y = 17
    solver.assert(&(&x + &_5 * &y)._eq(&_17));
    // x + y = 5
    solver.assert(&(&x + &y)._eq(&_5));

    match solver.check() {
        SatResult::Sat => {
            let model = solver.get_model();

            if let Some(m) = model {
                let x_val = m.get_const_interp(&x).unwrap().as_i64().unwrap();
                let y_val = m.get_const_interp(&y).unwrap().as_i64().unwrap();

                // x + 5 * y = 17
                // x + y = 5
                // => x = 2, y = 3
                assert_eq!(x_val, 2);
                assert_eq!(y_val, 3);

                println!("x: {}, y: {}", x_val, y_val);
            }
        }
        SatResult::Unsat => {
            println!("unsat");
            unreachable!();
        }
        SatResult::Unknown => {
            println!("unknown");
            unreachable!();
        }
    }
}

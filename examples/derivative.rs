use symbolica::{representations::Atom, state::State};

fn main() {
    let mut state = State::get_global_state().write().unwrap();

    let x = state.get_or_insert_var("x");
    let inputs = [
        "(1+2*x)^(5+x)",
        "log(2*x) + exp(3*x) + sin(4*x) + cos(y*x)",
        "f(x^2,x)",
        "der(0,1,f(x,x^3))",
    ];

    for input in inputs {
        let input = Atom::parse(input, &mut state).unwrap();

        let a = input.derivative(x);

        println!("d({})/dx = {}:", input, a);
    }
}

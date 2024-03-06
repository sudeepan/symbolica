use symbolica::{
    coefficient::CoefficientView,
    domains::finite_field,
    id::{Condition, Match, MatchSettings, PatternRestriction},
    representations::{Atom, AtomView},
    state::State,
};
fn main() {
    let mut state = State::get_global_state().write().unwrap();

    let expr = Atom::parse("f(1,2,3,4,5,6,7)", &mut state).unwrap();
    let pat_expr = Atom::parse("f(x__,y__,z__,w__)", &mut state).unwrap();

    let pattern = pat_expr.as_view().into_pattern();

    let x = state.get_or_insert_var("x__");
    let y = state.get_or_insert_var("y__");
    let z = state.get_or_insert_var("z__");
    let w = state.get_or_insert_var("w__");

    let conditions = Condition::from((x, PatternRestriction::Length(0, Some(2))))
        & (y, PatternRestriction::Length(0, Some(4)))
        & (
            y,
            PatternRestriction::Cmp(
                x,
                Box::new(|y, x| {
                    let len_x = match x {
                        Match::Multiple(_, s) => s.len(),
                        _ => 1,
                    };
                    let len_y = match y {
                        Match::Multiple(_, s) => s.len(),
                        _ => 1,
                    };
                    len_x >= len_y
                }),
            ),
        )
        & (
            z,
            PatternRestriction::Filter(Box::new(|x: &Match| {
                if let Match::Single(AtomView::Num(num)) = x {
                    if let CoefficientView::Natural(x, y) = num.get_coeff_view() {
                        y == 1 && x > 0 && finite_field::is_prime_u64(x as u64)
                    } else {
                        false
                    }
                } else {
                    false
                }
            })),
        )
        & (w, PatternRestriction::Length(0, None));
    let settings = MatchSettings::default();

    println!(
        "> Matching pattern {} : 0 <= len(x) <= 2, 0 <= len(y) <= 4, len(x) >= len(y) & is_prime(z) to {}:",
        pat_expr,
        expr
    );

    let mut it = pattern.pattern_match(expr.as_view(), &conditions, &settings);
    while let Some((location, used_flags, _atom, match_stack)) = it.next() {
        println!("\tMatch at location {:?} - {:?}:", location, used_flags);
        for (id, v) in match_stack {
            print!("\t\t{} = ", State::get_name(*id));
            match v {
                Match::Single(s) => {
                    print!("{}", s)
                }
                Match::Multiple(slice_type, mm) => {
                    print!("{:?} ", slice_type);
                    for vv in mm {
                        print!("{}", vv);
                        print!(", ")
                    }
                }
                Match::FunctionName(f) => {
                    print!("Fn {}", State::get_name(*f))
                }
            }
            println!();
        }
    }
}

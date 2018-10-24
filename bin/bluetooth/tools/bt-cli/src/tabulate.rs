use std::cmp;
use std::fmt::Write;

fn main() {
    //println!("Hello, world!");
    foo();
}

/// Trait representing the abstract algebra concept of a
/// Semigroup - an associative binary operation `mappend`
/// Semigroup's do not have an identity element - they are
/// `Nonoid`s minus identity.
trait Semigroup {
    fn mappend(a: Self, b: Self) -> Self;
}

/// Trait representing the abstract algebra concept of a
/// Monoid - an associative binary operation `mappend` with
/// an identity element `mzero`. This abstracts over 'summing',
/// including addition, multiplication, concatenation, minimum
/// and maximum
trait Monoid : Semigroup {
    fn mzero() -> Self;
}

// Homogeneous Tuples
struct Tuple2<T> (T,T);
struct Tuple3<T> (T,T,T);

impl<T> Tuple3<T> {
    fn map<U, F>(&self, f: F) -> Tuple3<U> where
        F : Fn(&T) -> U {
        Tuple3(
            f(&self.0),
            f(&self.1),
            f(&self.2)
        )
    }

    fn zip_with<U, V, F>(self, t2: Tuple3<U>, f: F) -> Tuple3<V> where
        F : Fn(T,U) -> V {
        Tuple3(
            f(self.0, t2.0),
            f(self.1, t2.1),
            f(self.2, t2.2),
        )
    }
}

impl<T: Semigroup> Semigroup for Tuple3<T> {
    fn mappend(a: Tuple3<T>, b: Tuple3<T>) -> Tuple3<T> {
        Tuple3(
            T::mappend(a.0, b.0),
            T::mappend(a.1, b.1),
            T::mappend(a.2, b.2),
        )
    }
}

impl<T: Monoid> Monoid for Tuple3<T> {
    fn mzero() -> Tuple3<T> { 
        Tuple3( T::mzero(), T::mzero(), T::mzero() )
    }
}

/// Newtype wrapper invoking the `Max` monoid
#[repr(transparent)]
struct Max( usize );

impl Semigroup for Max {
    fn mappend(a: Max, b: Max) -> Max {
        Max(cmp::max(a.0, b.0)) 
    }
}

impl Monoid for Max {
    fn mzero() -> Max {
        Max( 0 ) 
    }
}

/// Newtype wrapper invoking the `zip` monoid
struct Zip<T>( Vec<T> );

impl<T : Monoid> Semigroup for Zip<T> {
    fn mappend(a: Zip<T>, b: Zip<T>) -> Zip<T> {
        Zip( vec![] )
    }
}


impl<T : Monoid> Monoid for Zip<T> {
    fn mzero() -> Zip<T> {
        Zip( vec![] )
    }
}

fn msum<I : Iterator<Item = T>, T : Monoid>(i: I) -> T {
    let mut t: T = T::mzero();
    for elem in i {
        t = T::mappend(t, elem);
    }
    t
}
    
fn foo() {
    let mut string = String::new();
    let headers = Tuple3("Title".to_string(), "Detail".to_string(), "Other".to_string());
    let rows : Vec<Tuple3<String>> = vec![
        Tuple3("this one goes on for a while".to_string(), "analyze".to_string(), "supreme".to_string()),
        Tuple3("autonomous".to_string(), "ferocious".to_string(), "black".to_string()),
        Tuple3("red".to_string(), "equananimous".to_string(), "milquetoast".to_string()),
        Tuple3("fecundity".to_string(), "in order of size".to_string(), "this".to_string())
    ];
    let length = |s:&String| { Max(s.len()) };
    let col_lengths = Tuple3::<Max>::mappend(headers.map(length), msum(rows.iter().map(|row| row.map(length))));

    write!(string, "{:width$}", headers.0, width = (col_lengths.0).0 + 1);
    write!(string, "{:width$}", headers.1, width = (col_lengths.1).0+ 1);
    writeln!(string, "{:width$}", headers.2, width = (col_lengths.2).0 + 1);

    writeln!(string, "{:=<width$}", "", width = (col_lengths.0).0 + (col_lengths.1).0 + (col_lengths.2).0 + 3);
    for row in rows {
        write!(string, "{:width$}", row.0, width = (col_lengths.0).0 + 1);
        write!(string, "{:width$}", row.1, width = (col_lengths.1).0+ 1);
        writeln!(string, "{:width$}", row.2, width = (col_lengths.2).0 + 1);
    }
    println!("{}", string);
}

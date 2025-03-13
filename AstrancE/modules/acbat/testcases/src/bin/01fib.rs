fn main() {
    for i in 1..=10 {
        println!("fib({}) = {}", i, fib(i));
    }
}

fn fib(n: u32) -> u32 {
    if n == 1 || n == 2 {
        1
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

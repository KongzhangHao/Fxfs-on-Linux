use fxfs::platform::linux::testing::*;

fn main() {
    test_mkdir();
    test_rmdir();

    println!("All Tests Passed!");
}

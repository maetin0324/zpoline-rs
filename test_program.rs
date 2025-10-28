fn main() {
    println!("=== Test Program Start ===");

    // Test write
    println!("1. Hello, World!");
    println!("2. This is a test message.");

    // Test getpid
    let pid = std::process::id();
    println!("3. My process ID is: {}", pid);

    // Test file read
    if let Ok(contents) = std::fs::read_to_string("/etc/hostname") {
        println!("4. Hostname: {}", contents.trim());
    }

    println!("=== Test Program End ===");
}

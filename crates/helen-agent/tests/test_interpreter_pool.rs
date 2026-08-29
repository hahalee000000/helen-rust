//! Tests for interpreter pooling

use helen_agent::actor_bridge::pool::InterpreterPool;

#[test]
fn test_pool_creation() {
    let pool = InterpreterPool::new(3);
    assert_eq!(pool.size(), 3);
    assert_eq!(pool.available(), 3);
}

#[test]
fn test_pool_checkout() {
    let pool = InterpreterPool::new(2);
    
    let interp1 = pool.checkout();
    assert!(interp1.is_some());
    assert_eq!(pool.available(), 1);
    
    let interp2 = pool.checkout();
    assert!(interp2.is_some());
    assert_eq!(pool.available(), 0);
    
    // Pool exhausted
    let interp3 = pool.checkout();
    assert!(interp3.is_none());
}

#[test]
fn test_pool_checkin() {
    let pool = InterpreterPool::new(2);
    
    let interp1 = pool.checkout().unwrap();
    assert_eq!(pool.available(), 1);
    
    pool.checkin(interp1);
    assert_eq!(pool.available(), 2);
}

#[test]
fn test_pool_reuse() {
    let pool = InterpreterPool::new(1);
    
    let interp1 = pool.checkout().unwrap();
    pool.checkin(interp1);
    
    let _interp2 = pool.checkout().unwrap();
    // Should get the same interpreter back
    assert_eq!(pool.available(), 0);
}

#[test]
fn test_pool_zero_size() {
    let pool = InterpreterPool::new(0);
    assert_eq!(pool.size(), 0);
    assert_eq!(pool.available(), 0);
    assert!(pool.checkout().is_none());
}

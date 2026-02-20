use std::fmt;

// New-type for OrderId to avoid mixing with plain integers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OrderId(u64);

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

// Each state is a distinct type — impossible to skip states at compile time
struct NewOrder { id: OrderId }
struct PaidOrder { id: OrderId }
struct ShippedOrder { id: OrderId }

impl NewOrder {
    fn new(id: OrderId) -> Self { Self { id } }
    fn pay(self) -> PaidOrder { PaidOrder { id: self.id } }
}

impl PaidOrder {
    fn ship(self) -> ShippedOrder { ShippedOrder { id: self.id } }
}

impl fmt::Display for NewOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NewOrder({})", self.id)
    }
}
impl fmt::Display for PaidOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PaidOrder({})", self.id)
    }
}
impl fmt::Display for ShippedOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ShippedOrder({})", self.id)
    }
}

fn main() {
    // Create a new order with a typed OrderId
    let id = OrderId(1001);
    let order = NewOrder::new(id);
    println!("Created: {}", order);

    // Pay the order — now we have a PaidOrder; cannot call `ship` on NewOrder
    let order = order.pay();
    println!("After pay: {}", order);

    // Ship the order — now we have a ShippedOrder
    let order = order.ship();
    println!("After ship: {}", order);

    // The compiler will prevent illegal transitions such as:
    // let illegal = NewOrder::new(id).ship(); // error: no method `ship` on `NewOrder`
}

use tarpc::{
    client, context,
    server::{self, Handler},
};
use std::io;



// This is the service definition. It looks a lot like a trait definition.
// It defines one RPC, hello, which takes one arg, name, and returns a String.
tarpc::service! {
    /// Returns a greeting for name.
    rpc hello(name: String) -> String;
}
pub type NodeId = usize;
pub const ROOT_ID: NodeId = 1;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum NodeType {
    Dir,
    File,
    SymLink,
}

// mapping from disk file
pub struct Node {
    typ: NodeType,
    ino: NodeId,
    size: u64
}

pub enum FileType {
    Null, // unknown
    Node,
    Pipe
}

// opened file 
pub trait File {
    fn get_node(&self) -> &Node;
}

pub trait FileSystem {

}

use serde::{Deserialize, Serialize};

macro_rules! identity {
    ($name:ident) => {
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub u64);
    };
}
identity!(WorldId);
identity!(PlayerId);
identity!(WorldSeed);

// Assignment 2, Part 1: Typestate pattern for Post

pub struct NewPost {
    pub content: String,
}

pub struct UnmoderatedPost {
    pub content: String,
}

pub struct PublishedPost {
    pub content: String,
}

pub struct DeletedPost;

impl NewPost {
    pub fn publish(self) -> UnmoderatedPost {
        UnmoderatedPost { content: self.content }
    }
}

impl UnmoderatedPost {
    pub fn allow(self) -> PublishedPost {
        PublishedPost { content: self.content }
    }
    pub fn deny(self) -> DeletedPost {
        DeletedPost
    }
}

impl PublishedPost {
    pub fn delete(self) -> DeletedPost {
        DeletedPost
    }
}

// No delete() for NewPost, no deny() for DeletedPost, etc.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typestate_transitions() {
        let new = NewPost { content: "Hello".to_string() };
        let unmod = new.publish();
        let published = unmod.allow();
        let _deleted = published.delete();
    }

    #[test]
    fn test_deny_transition() {
        let new = NewPost { content: "Test".to_string() };
        let unmod = new.publish();
        let _deleted = unmod.deny();
    }
}

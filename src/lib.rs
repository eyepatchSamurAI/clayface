use futures::future::{BoxFuture, FutureExt};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Weak},
};
use tokio::sync::{Mutex};

use uuid::Uuid;

pub trait Message: Send + Sync {}
pub trait Actor: Send + Sync {
    type Message: Message;

    fn handle_message(
        &mut self,
        msg: Self::Message,
    ) -> BoxFuture<'static, Result<(), SupervisorError>>;
    // fn handle_message(&mut self, msg: Self::Message) -> BoxFuture<'static, ()>;
}

#[derive(Debug)]
enum ActorError {
    Error,
}
pub struct ActorRef<A: Actor>
where
    A: Actor + Send + Sync,
{
    address: Uuid,
    actor: Arc<Mutex<A>>,
    // parent: Option<Weak<Mutex<dyn Actor<Message = A::Message>>>>,
    parent: Option<Weak<Mutex<A>>>,
    children: Vec<Arc<Mutex<A>>>,
    mailbox: VecDeque<A::Message>,
}

impl<A: Actor + Send + Sync> ActorRef<A> {
    pub fn new(actor: A, parent: Option<Weak<Mutex<A>>>) -> Arc<Mutex<Self>> {
        let actor_ref = Arc::new(Mutex::new(Self {
            address: Uuid::new_v4(),
            actor: Arc::new(Mutex::new(actor)),
            parent,
            children: Vec::new(),
            mailbox: VecDeque::new(),
        }));

        actor_ref
    }

    pub fn add_to_mailbox(&mut self, msg: A::Message) {
        self.mailbox.push_back(msg)
    }

    pub async fn process_mailbox(&mut self) -> Result<(), ActorError> {
        while let Some(msg) = self.mailbox.pop_front() {
            let mut actor_guard = self.actor.lock().await;
            actor_guard.handle_message(msg).await;
            return Ok(());
            // if let Ok(mut actor_guard) = self.actor.lock() {
            //     actor_guard.handle_message(msg).await;
            //     return Ok(());
            // } else {
            //     return Err(ActorError::Error);
            // }
        }
        Ok(())
    }
}

struct Supervisor<A>
where
    A: Actor,
{
    children: HashMap<Uuid, Arc<Mutex<ActorRef<A>>>>,
}

enum SupervisoryMessage<A: Actor> {
    // RouteToAny {
    //     message: A::Message
    // }
    RouteToChild { address: Uuid, message: A::Message },
}

impl<A: Actor> Message for SupervisoryMessage<A> {}

enum SupervisorError {
    ChildActorNotFound,
}

impl<A> Actor for Supervisor<A>
where
    A: Actor,
{
    type Message = SupervisoryMessage<A>;

    // fn handle_message(
    //     &mut self,
    //     msg: Self::Message,
    // ) -> BoxFuture<'static, Result<(), SupervisorError>> {
    //     match msg {
    //         SupervisoryMessage::RouteToChild { address, message } => {
    //             (async move {
    //                 // Step 3: Optionally trigger message processing
    //                 tokio::spawn(async {
    //                     if let Some(actor_ref) = self.children.get(&address) {
    //                         // Step 2: Send message to child actor
    //                         actor_ref.add_to_mailbox(message);
    //                         actor_ref.process_mailbox().await;
    //                         return Ok(());
    //                     }
    //                     Err(SupervisorError::ChildActorNotFound)
    //                 });
    //             })
    //             .boxed()
    //         }
    //     }
    //     // ... other match arms
    // }

    fn handle_message(&mut self, msg: Self::Message) -> BoxFuture<'static, Result<(), SupervisorError>> {
        let children_clone = self.children.clone(); // Clone HashMap
        
        (async move {
            match msg {
                SupervisoryMessage::RouteToChild { address, message } => {
                    let actor_ref_clone = if let Some(actor_ref) = children_clone.get(&address) {
                        actor_ref.clone()
                    } else {
                        return Err(SupervisorError::ChildActorNotFound);
                    };
    
                    let mut actor_ref_locked = actor_ref_clone.lock().await;
                    actor_ref_locked.add_to_mailbox(message);
    
                    // Spawn a new task for message processing
                    tokio::spawn(async move {
                        let mut actor_ref_locked = actor_ref_clone.lock().await;
                        actor_ref_locked.process_mailbox().await.expect("Error processing mailbox");
                    });
    
                    Ok(())
                }
                // ... other match arms
            }
        })
        .boxed()
    }
    
    
}

// #[derive(Debug, Clone)]
// struct Scheduler {
//     root_guardian: Arc<Mutex<Actor>>
// }

// impl Scheduler {
//     fn new() -> Self {
//         let root_actor = Arc::new(Mutex::new(Actor::new(None)));
//         // Locking should never fail at start
//         root_actor.lock().expect("Failed to lock root actor").spawn(&Arc::downgrade(&root_actor)); // user
//         root_actor.lock().expect("Failed to lock root actor").spawn(&Arc::downgrade(&root_actor)); // system

//         Scheduler { root_guardian: root_actor }
//     }

//     fn get_user_guardian(&self) -> Option<Arc<Mutex<Actor>>> {
//         if let Ok(actor_guard) = self.root_guardian.lock() {
//             return actor_guard.children.first().cloned();
//         }
//         None
//     }
//     fn get_system_guardian(&self) -> Option<Arc<Mutex<Actor>>> {
//         if let Ok(actor_guard) = self.root_guardian.lock() {
//             return actor_guard.children.get(1).cloned();
//         }
//         None
//     }

// }

// #[derive(Debug, Clone)]
// struct Actor {
//     address: Uuid,
//     queue: VecDeque<Message>,
//     children: Vec<Arc<Mutex<Actor>>>,
//     parent: Option<Weak<Mutex<Actor>>>
// }

// impl PartialEq for Actor {
//     fn eq(&self, other: &Self) -> bool {
//         self.address == other.address
//     }
//     fn ne(&self, other: &Self) -> bool {
//         self.address != other.address
//     }
// }

// impl Actor {
//     fn new(parent: Option<Weak<Mutex<Actor>>>) -> Self {
//         Actor { address: Uuid::new_v4(), queue: VecDeque::new(), children: Vec::new(), parent }
//     }

//     fn add_message(&mut self, message: Message) {
//         self.queue.push_back(message);
//     }
//     fn next_message(&mut self) -> Option<Message> {
//         self.queue.pop_front()
//     }

//     fn spawn(&self, actor: &Weak<Mutex<Actor>>) {
//         let parent_arc = actor.upgrade().expect("Parent actor has been dropped"); // Does add one to rc

//         let child = Actor::new(Some(actor.clone()));
//         let mut parent_actor_guard = parent_arc.lock().unwrap();
//         parent_actor_guard.children.push(Arc::new(Mutex::new(child)));
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn playground() {
    //     let scheduler = Scheduler::new();
    // }
}

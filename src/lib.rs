use futures::future::{BoxFuture, FutureExt};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Weak},
};
use tokio::sync::Mutex;

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

    RouteToChild { address: Uuid, message: A::Message },
}

impl<A: Actor> Message for SupervisoryMessage<A> {}

pub enum SupervisorError {
    ChildActorNotFound,
}

impl<A> Actor for Supervisor<A>
where
    A: Actor + 'static,
{
    type Message = SupervisoryMessage<A>;

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
    
                    // Clone the actor_ref_clone before moving into the spawned task
                    let actor_ref_clone_for_task = actor_ref_clone.clone();
    
                    let mut actor_ref_locked = actor_ref_clone.lock().await;
                    actor_ref_locked.add_to_mailbox(message);
    
                    // Spawn a new task for message processing
                    tokio::spawn(async move {
                        let mut actor_ref_locked = actor_ref_clone_for_task.lock().await;
                        actor_ref_locked.process_mailbox().await.expect("Error processing mailbox");
                    });
    
                    Ok(())
                }
            }
        })
        .boxed()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn playground() {
    //     let scheduler = Scheduler::new();
    // }
}

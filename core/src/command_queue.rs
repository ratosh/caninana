use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

#[derive(Debug, Clone)]
struct InnerQueue {
    pub command: Command,
    pub blocked: bool,
    pub priority: usize,
}

impl InnerQueue {
    fn new(command: Command, blocked: bool, priority: usize) -> Self {
        Self {
            command,
            blocked,
            priority,
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum Command {
    UnitCommand {
        unit_type: UnitTypeId,
        wanted_amount: usize,
        save_resources: bool,
    },
    UpgradeCommand {
        upgrade: UpgradeId,
        save_resources: bool,
    },
}

impl Command {
    pub fn new_unit(unit_type: UnitTypeId, wanted_amount: usize, save_resources: bool) -> Self {
        Command::UnitCommand {
            unit_type,
            wanted_amount,
            save_resources,
        }
    }

    pub fn new_upgrade(upgrade: UpgradeId, save_resources: bool) -> Self {
        Command::UpgradeCommand {
            upgrade,
            save_resources,
        }
    }
}

pub struct CommandQueueIter {
    queue: Vec<InnerQueue>,
    index: usize,
}

#[derive(Debug, Default)]
pub struct CommandQueue {
    queue: Vec<InnerQueue>,
}

impl CommandQueue {
    pub fn print_queue(&self) {
        println!("----------------------------------");
        for command in self.queue.iter() {
            println!("Pending command {:?}", command);
        }
    }

    pub fn check_completion(&mut self, bot: &Bot) {
        self.queue.retain(|x| match x.command {
            Command::UnitCommand {
                unit_type,
                wanted_amount,
                save_resources: _,
            } => bot.counter().all().count(unit_type) < wanted_amount,
            Command::UpgradeCommand {
                upgrade,
                save_resources: _,
            } => !bot.has_upgrade(upgrade) && !bot.is_ordered_upgrade(upgrade),
        });
    }

    pub fn push(&mut self, command: Command, blocked: bool, priority: usize) {
        let replace_previous_command = self.queue.iter().position(|i| match &i.command {
            Command::UnitCommand {
                unit_type,
                wanted_amount: _,
                save_resources: _,
            } => match command {
                Command::UnitCommand {
                    unit_type: new_type,
                    wanted_amount: _,
                    save_resources: _,
                } => !i.blocked && *unit_type == new_type && i.priority == priority,
                _ => false,
            },
            Command::UpgradeCommand {
                upgrade,
                save_resources: _,
            } => match command {
                Command::UpgradeCommand {
                    upgrade: new_upgrade,
                    save_resources: _,
                } => *upgrade == new_upgrade,
                _ => false,
            },
        });
        if let Some(previous_command_index) = replace_previous_command {
            self.queue.remove(previous_command_index);
        }
        let index = self.queue.iter().position(|i| i.priority < priority);
        let item = InnerQueue::new(command, blocked, priority);
        if let Some(found_index) = index {
            self.queue.insert(found_index, item);
        } else {
            self.queue.push(item);
        }
    }
}

impl IntoIterator for &CommandQueue {
    type Item = Command;
    type IntoIter = CommandQueueIter;

    fn into_iter(self) -> Self::IntoIter {
        CommandQueueIter {
            index: 0,
            queue: self.queue.to_vec(),
        }
    }
}

impl Iterator for CommandQueueIter {
    type Item = Command;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.queue.len() {
            let item = self.queue[self.index].clone();
            self.index += 1;
            return Some(item.command);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use rust_sc2::prelude::*;

    use crate::command_queue::Command;
    use crate::CommandQueue;

    #[test]
    fn iterator_same_command_ignored() {
        let mut queue = CommandQueue::default();
        let command1 = Command::new_unit(UnitTypeId::Zergling, 10, false);
        queue.push(command1.clone(), false, 0);
        queue.push(command1.clone(), false, 0);
        let mut iter = queue.into_iter();
        let next = iter.next();
        let next2 = iter.next();
        assert_eq!(next.is_some(), true);
        assert_eq!(next2.is_some(), false);
    }

    #[test]
    fn iterator_more_units_equal_priority_replaced() {
        let mut queue = CommandQueue::default();
        let command1 = Command::new_unit(UnitTypeId::Zergling, 10, false);
        let command2 = Command::new_unit(UnitTypeId::Zergling, 20, false);
        queue.push(command1, false, 0);
        queue.push(command2.clone(), false, 0);
        let mut iter = queue.into_iter();
        let next = iter.next();
        assert_eq!(next.is_some(), true);
        let mut iter = queue.into_iter();
        let next = iter.next();
        let expected = command2;
        assert_eq!(next.unwrap(), expected);
        let next2 = iter.next();
        assert_eq!(next2.is_some(), false);
    }

    #[test]
    fn iterator_less_units_equal_priority_replaced() {
        let mut queue = CommandQueue::default();
        let command1 = Command::new_unit(UnitTypeId::Zergling, 20, false);
        let command2 = Command::new_unit(UnitTypeId::Zergling, 10, false);
        queue.push(command1, false, 5);
        queue.push(command2.clone(), false, 5);
        let mut iter = queue.into_iter();
        let next = iter.next();
        assert_eq!(next.is_some(), true);
        let mut iter = queue.into_iter();
        let next = iter.next();
        let expected = command2;
        assert_eq!(next.unwrap(), expected);
        let next2 = iter.next();
        assert_eq!(next2.is_some(), false);
    }

    #[test]
    fn iterator_high_priority_check() {
        let mut queue = CommandQueue::default();
        let command1 = Command::new_unit(UnitTypeId::Zergling, 10, false);
        let command2 = Command::new_unit(UnitTypeId::Zergling, 20, false);
        queue.push(command1, false, 0);
        queue.push(command2.clone(), false, 1);
        let next = queue.into_iter().next();
        let expected = command2;
        assert_eq!(next.is_some(), true);
        assert_eq!(next.clone().unwrap(), expected);
    }

    #[test]
    fn iterator_blocked_check() {
        let mut queue = CommandQueue::default();
        let command1 = Command::new_unit(UnitTypeId::Zergling, 10, false);
        let command2 = Command::new_unit(UnitTypeId::Zergling, 20, false);
        queue.push(command1, true, 1);
        queue.push(command2, false, 0);
        let mut iter = queue.into_iter();
        let next = iter.next();
        let next2 = iter.next();
        assert_eq!(next.is_some(), true);
        assert_eq!(next2.is_some(), true);
    }
}

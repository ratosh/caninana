// Disable warning for the crate name, not a really good way to do this but..
// (https://github.com/rust-lang/rust/issues/45127)
#![allow(non_snake_case)]

#[macro_use]
extern crate clap;

use std::ops::RangeInclusive;
use std::str::FromStr;

mod bot;

use crate::bot::Caninana;
use clap::ArgEnum;
use rand::prelude::SliceRandom;
use rand::thread_rng;
use rust_sc2::prelude::*;

use crate::clap::Parser;

const PORT_RANGE: RangeInclusive<i32> = 1..=65535;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Option<Command>,

    #[clap(long = "LadderServer", parse(try_from_str))]
    ladder_server: Option<String>,

    #[clap(long = "GamePort", validator = port_in_range)]
    game_port: Option<i32>,

    #[clap(long = "OpponentId", parse(try_from_str))]
    opponent: Option<String>,

    #[clap(long = "StartPort", validator = port_in_range)]
    start_port: Option<i32>,

    #[clap(long, short, arg_enum)]
    race: Option<GameRace>,

    /// Set game step for bot
    #[clap(short = 's', long = "step", default_value_t = 1)]
    game_step: u32,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
pub enum GameRace {
    Terran,
    Zerg,
    Protoss,
    Random,
}

impl From<GameRace> for Race {
    fn from(race: GameRace) -> Self {
        match race {
            GameRace::Terran => Race::Terran,
            GameRace::Zerg => Race::Zerg,
            GameRace::Protoss => Race::Protoss,
            GameRace::Random => Race::Random,
        }
    }
}

fn port_in_range(s: &str) -> Result<(), String> {
    i32::from_str(s)
        .map(|port| PORT_RANGE.contains(&port))
        .map_err(|e| e.to_string())
        .and_then(|result| match result {
            true => Ok(()),
            false => Err(format!(
                "Port not in range {}-{}",
                PORT_RANGE.start(),
                PORT_RANGE.end()
            )),
        })
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Bot versus in-game AI
    Local {
        map: Option<String>,
        race: Option<Race>,
        difficulty: Option<Difficulty>,
        build: Option<AIBuild>,
        sc2_version: Option<String>,
        save_replay: Option<String>,
        realtime: Option<bool>,
    },
    /// Bot versus Human
    Human {
        /// Specify a map
        #[clap(long("map"), short('m'))]
        map: Option<String>,
        /// Sets human race
        #[clap(long("race"), short('r'))]
        race: Option<Race>,
        /// Sets human name
        name: Option<String>,
        /// Sets sc2 version
        sc2_version: Option<String>,
        /// Replay file
        save_replay: Option<String>,
    },
}

fn main() -> SC2Result<()> {
    env_logger::init();
    let app = Cli::parse();
    let game_step = app.game_step;

    let mut bot = Caninana::default();
    bot.set_game_step(game_step);
    if let Some(race) = app.race {
        bot.race = race.into();
    }

    const LADDER_MAPS: &[&str] = &[
        "2000AtmospheresAIE",
        "BerlingradAIE",
        "BlackburnAIE",
        "CuriousMindsAIE",
        "GlitteringAshesAIE",
        "HardwireAIE",
    ];
    let mut rng = thread_rng();

    match &app.command {
        Some(Command::Local {
            map,
            race,
            difficulty,
            build,
            sc2_version,
            save_replay,
            realtime,
        }) => run_vs_computer(
            &mut bot,
            Computer::new(
                race.unwrap_or(Race::Random),
                difficulty.unwrap_or(Difficulty::CheatMoney),
                *build,
            ),
            map.clone()
                .unwrap_or_else(|| LADDER_MAPS.choose(&mut rng).unwrap().to_string())
                .as_str(),
            LaunchOptions {
                sc2_version: sc2_version.as_deref(),
                realtime: realtime.unwrap_or_default(),
                save_replay_as: save_replay.as_deref(),
            },
        ),
        Some(Command::Human {
            map,
            race,
            name,
            sc2_version,
            save_replay,
        }) => run_vs_human(
            &mut bot,
            PlayerSettings {
                race: race.unwrap_or(Race::Random),
                name: name.as_deref(),
                ..Default::default()
            },
            map.clone()
                .unwrap_or_else(|| LADDER_MAPS.choose(&mut rng).unwrap().to_string())
                .as_str(),
            LaunchOptions {
                sc2_version: sc2_version.as_deref(),
                realtime: true,
                save_replay_as: save_replay.as_deref(),
            },
        ),
        _ => run_ladder_game(
            &mut bot,
            app.ladder_server.unwrap().as_str(),
            app.game_port.unwrap(),
            app.start_port.unwrap(),
            app.opponent.as_deref(),
        ),
    }
}

// Disable warning for the crate name, not a really good way to do this but..
// (https://github.com/rust-lang/rust/issues/45127)
#![allow(non_snake_case)]

#[macro_use]
extern crate clap;

use caninana_core::*;
use rand::prelude::*;
use rust_sc2::prelude::*;

use caninana_core::managers::army_manager::ArmyManager;
use caninana_core::managers::production_manager::ProductionManager;
use caninana_core::managers::queen_manager::QueenManager;
use caninana_core::managers::ravager_manager::RavagerManager;
use caninana_core::managers::resource_manager::ResourceManager;
use caninana_core::managers::worker_manager::WorkerManager;

use caninana_openings::zerg::poolfirst::PoolFirst;

fn main() -> SC2Result<()> {
    env_logger::init();
    let app = clap_app!(DebugBot =>
        (version: crate_version!())
        (author: crate_authors!())
        (@arg ladder_server: --LadderServer +takes_value)
        (@arg opponent_id: --OpponentId +takes_value)
        (@arg host_port: --GamePort +takes_value)
        (@arg player_port: --StartPort +takes_value)
        (@arg race: -r --race
            +takes_value
            "Sets race for debug bot"
        )
        (@arg game_step: -s --step
            +takes_value
            default_value("1")
            "Sets game step for bot"
        )
        (@subcommand local =>
            (about: "Runs local game vs Computer")
            (@arg map: -m --map
                +takes_value
            )
            (@arg race: -r --race
                +takes_value
                "Sets opponent race"
            )
            (@arg difficulty: -d --difficulty
                +takes_value
                "Sets opponent diffuculty"
            )
            (@arg ai_build: --("ai-build")
                +takes_value
                "Sets opponent build"
            )
            (@arg sc2_version: --("sc2-version")
                +takes_value
                "Sets sc2 version"
            )
            (@arg save_replay: --("save-replay")
                +takes_value
                "Sets path to save replay"
            )
            (@arg realtime: --realtime "Enables realtime mode")
        )
        (@subcommand human =>
            (about: "Runs game Human vs Bot")
            (@arg map: -m --map
                +takes_value
            )
            (@arg race: -r --race *
                +takes_value
                "Sets human race"
            )
            (@arg name: --name
                +takes_value
                "Sets human name"
            )
            (@arg sc2_version: --("sc2-version")
                +takes_value
                "Sets sc2 version"
            )
            (@arg save_replay: --("save-replay")
                +takes_value
                "Sets path to save replay"
            )
        )
    )
    .get_matches();

    let game_step = match app.value_of("game_step") {
        Some("0") => panic!("game_step must be X >= 1"),
        Some(step) => step.parse::<u32>().expect("Can't parse game_step"),
        None => unreachable!(),
    };

    let mut bot = Caninana::default();
    bot.set_game_step(game_step);
    if let Some(race) = app
        .value_of("race")
        .map(|race| race.parse().expect("Can't parse bot race"))
    {
        bot.race = race;
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

    match app.subcommand() {
        ("local", Some(sub)) => run_vs_computer(
            &mut bot,
            Computer::new(
                sub.value_of("race").map_or(Race::Random, |race| {
                    race.parse().expect("Can't parse computer race")
                }),
                sub.value_of("difficulty")
                    .map_or(Difficulty::CheatMoney, |difficulty| {
                        difficulty.parse().expect("Can't parse computer difficulty")
                    }),
                sub.value_of("ai_build")
                    .map(|ai_build| ai_build.parse().expect("Can't parse computer build")),
            ),
            sub.value_of("map")
                .unwrap_or_else(|| LADDER_MAPS.choose(&mut rng).unwrap()),
            LaunchOptions {
                sc2_version: sub.value_of("sc2_version"),
                realtime: sub.is_present("realtime"),
                save_replay_as: sub.value_of("save_replay"),
            },
        ),
        ("human", Some(sub)) => run_vs_human(
            &mut bot,
            PlayerSettings {
                race: sub
                    .value_of("race")
                    .unwrap()
                    .parse()
                    .expect("Can't parse human race"),
                name: sub.value_of("name"),
                ..Default::default()
            },
            sub.value_of("map")
                .unwrap_or_else(|| LADDER_MAPS.choose(&mut rng).unwrap()),
            LaunchOptions {
                sc2_version: sub.value_of("sc2_version"),
                realtime: false,
                save_replay_as: sub.value_of("save_replay"),
            },
        ),
        _ => run_ladder_game(
            &mut bot,
            app.value_of("ladder_server").unwrap_or("127.0.0.1"),
            app.value_of("host_port")
                .expect("GamePort must be specified"),
            app.value_of("player_port")
                .expect("StartPort must be specified")
                .parse()
                .expect("Can't parse StartPort"),
            app.value_of("opponent_id"),
        ),
    }
}

#[bot]
#[derive(Default)]
struct Caninana {
    army_manager: ArmyManager,
    production_manager: ProductionManager,
    ravager_manager: RavagerManager,
    queen_manager: QueenManager,
    resource_manager: ResourceManager,
    worker_manager: WorkerManager,
    opening: PoolFirst,
    bot_info: BotInfo,
}

impl Player for Caninana {
    fn get_player_settings(&self) -> PlayerSettings {
        PlayerSettings::new(Race::Zerg).with_name("Caninana")
    }

    fn on_start(&mut self) -> SC2Result<()> {
        self.opening.opening(&self._bot, &mut self.bot_info);
        Ok(())
    }

    fn on_step(&mut self, _iteration: usize) -> SC2Result<()> {
        self.army_manager
            .process(&mut self._bot, &mut self.bot_info);
        self.production_manager
            .process(&mut self._bot, &mut self.bot_info);
        self.ravager_manager
            .process(&mut self._bot, &mut self.bot_info);
        self.queen_manager
            .process(&mut self._bot, &mut self.bot_info);
        self.resource_manager
            .process(&mut self._bot, &mut self.bot_info);
        self.worker_manager
            .process(&mut self._bot, &mut self.bot_info);
        Ok(())
    }

    /// Called once on last step with a result for your bot.
    fn on_end(&self, _result: GameResult) -> SC2Result<()> {
        println!("Result {:?}", _result);
        Ok(())
    }

    fn on_event(&mut self, event: Event) -> SC2Result<()> {
        self.army_manager.on_event(&event);
        self.worker_manager.on_event(&event);
        Ok(())
    }
}

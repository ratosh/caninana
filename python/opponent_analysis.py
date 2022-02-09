import json
import os

import requests

# Download
bot_ids = [386]  # Ids on AI ARENA
token = os.environ['ARENA_API_TOKEN']  # Environment variable with token from: https://aiarena.net/profile/token/
file_path = './replays/'
auth = {'Authorization': f'Token {token}'}
map_names = dict()


def retrieve_map_name(map_id):
    if map_id not in map_names:
        r = requests.get(f'https://aiarena.net/api/maps/{map_id}', headers=auth)
        data = json.loads(r.text)
        map_names[map_id] = data['name']
    return map_names[map_id]


if not os.path.exists(file_path):
    os.makedirs(file_path)
for bot_id in bot_ids:
    participation_address = f'https://aiarena.net/api/match-participations/?bot={bot_id}'
    matchup_elo = {}
    matchup_stats = {}
    map_stats = {}
    total_games = 0
    total_wins = 0
    while participation_address:
        participation_response = requests.get(participation_address, headers=auth)
        assert participation_response.status_code == 200, 'Unexpected status_code returned from match-participations'
        participation = json.loads(participation_response.text)
        participation_address = participation['next']
        for i in range(len(participation['results'])):
            match_id = participation['results'][i]['match']
            participant_number = participation['results'][i]['participant_number']
            result = participation['results'][i]['result']
            total_games += 1
            if result == 'win':
                total_wins += 1
            if participation['results'][i]['elo_change']:
                results_response = requests.get(f'https://aiarena.net/api/results/?match={match_id}', headers=auth)
                assert results_response.status_code == 200, 'Unexpected status_code returned from results'
                match_response = requests.get(f'https://aiarena.net/api/matches/{match_id}', headers=auth)
                assert match_response.status_code == 200, 'Unexpected status_code returned from results'
                result_details = json.loads(results_response.text)
                match_details = json.loads(match_response.text)
                bot1_name = result_details['results'][0]['bot1_name']
                bot2_name = result_details['results'][0]['bot2_name']
                map_name = retrieve_map_name(match_details['map'])
                enemy_name = bot1_name
                if participant_number == 1:
                    enemy_name = bot2_name
                if enemy_name not in matchup_elo:
                    matchup_elo[enemy_name] = 0
                    matchup_stats[enemy_name] = {'wins': 0, 'losses': 0, 'ties': 0}
                matchup_elo[enemy_name] += participation['results'][i]['elo_change']
                if map_name not in map_stats:
                    map_stats[map_name] = {'wins': 0, 'losses': 0, 'ties': 0, 'games': 0}
                if result == 'win':
                    matchup_stats[enemy_name]['wins'] += 1
                    map_stats[map_name]['wins'] += 1
                    map_stats[map_name]['games'] += 1
                elif result == 'loss':
                    matchup_stats[enemy_name]['losses'] += 1
                    map_stats[map_name]['losses'] += 1
                    map_stats[map_name]['games'] += 1
                elif result == 'tie':
                    matchup_stats[enemy_name]['ties'] += 1
                    map_stats[map_name]['ties'] += 1
                    map_stats[map_name]['games'] += 1
    print(f"{bot_id:<24}")
    print(f"{100*total_wins/total_games:.2f}% win rate after {total_games} games")
    print("")
    print(f"|{' Bot ':<20}|{' Elo ':<5}|{' Wins ':<6}|{' Losses ':<8}|{' Ties ':<6}|")
    print(f"|{'-'*20}|{'-'*5}|{'-'*6}|{'-'*8}|{'-'*6}|")
    sorted_map = {k: v for k, v in sorted(matchup_elo.items(), key=lambda item: item[1])[:5]}
    for key, value in sorted_map.items():
        stat = matchup_stats[key]
        print(f"| {key:<18} | {value:<3} | {stat['wins']:<4} | {stat['losses']:<6} | {stat['ties']:<4} |")
    print("")
    print(f"|{' Map stats ':<20}|{' Wins ':<6}|{' Losses ':<8}|{' Ties ':<6}| {' Win% ':<7}")
    print(f"|{'-'*20}|{'-'*6}|{'-'*8}|{'-'*6}|{'-'*7}|")
    for key, value in map_stats.items():
        print(f"| {key:<18} | {value['wins']:<4} | {value['losses']:<6} | {value['ties']:<4} | {100*value['wins']/value['games']:.2f}")


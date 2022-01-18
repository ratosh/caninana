import glob
import json
import os

import requests

# Download
bot_ids = [376]  # Ids on AI ARENA
token = os.environ['ARENA_API_TOKEN']  # Environment variable with token from: https://aiarena.net/profile/token/
file_path = './replays/'
auth = {'Authorization': f'Token {token}'}

if not os.path.exists(file_path):
    os.makedirs(file_path)
for bot_id in bot_ids:
    participation_address = f'https://aiarena.net/api/match-participations/?bot={bot_id}'
    elo_map = {}
    stat_map = {}
    total_games = 0
    total_wins = 0
    while participation_address:
        response = requests.get(participation_address, headers=auth)
        assert response.status_code == 200, 'Unexpected status_code returned from match-participations'
        participation = json.loads(response.text)
        participation_address = participation['next']
        for i in range(len(participation['results'])):
            match_id = participation['results'][i]['match']
            participant_number = participation['results'][i]['participant_number']
            result = participation['results'][i]['result']
            total_games += 1
            if result == 'win':
                total_wins += 1
            if participation['results'][i]['elo_change']:
                response = requests.get(f'https://aiarena.net/api/results/?match={match_id}', headers=auth)
                assert response.status_code == 200, 'Unexpected status_code returned from results'
                match_details = json.loads(response.text)
                bot1_name = match_details['results'][0]['bot1_name']
                bot2_name = match_details['results'][0]['bot2_name']
                enemy_name = bot1_name
                if participant_number == 1:
                    enemy_name = bot2_name
                if enemy_name not in elo_map:
                    elo_map[enemy_name] = 0
                    stat_map[enemy_name] = {'wins': 0, 'losses': 0, 'ties': 0}
                elo_map[enemy_name] += participation['results'][i]['elo_change']
                if result == 'win':
                    stat_map[enemy_name]['wins'] += 1
                elif result == 'loss':
                    stat_map[enemy_name]['losses'] += 1
                elif result == 'tie':
                    stat_map[enemy_name]['ties'] += 1
    print(f"{bot_id:<24}")
    print(f"{100*total_wins/total_games:.2f}% win rate after {total_games} games")
    print("")
    print(f"|{' Bot':<20}|{' Elo':<5}|{' Wins':<6}|{' Losses':<8}|{' Ties':<6}|")
    print(f"|{'-'*20}|{'-'*5}|{'-'*6}|{'-'*8}|{'-'*6}|")
    sorted_map = {k: v for k, v in sorted(elo_map.items(), key=lambda item: item[1])[:5]}
    for key, value in sorted_map.items():
        stat = stat_map[key]
        print(f"| {key:<18} | {value:<3} | {stat['wins']:<4} | {stat['losses']:<6} | {stat['ties']:<4} |")

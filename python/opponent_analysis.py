import glob
import json
import os

import requests

# Download
bot_ids = [375]  # Ids on AI ARENA
token = os.environ['ARENA_API_TOKEN']  # Environment variable with token from: https://aiarena.net/profile/token/
file_path = './replays/'
auth = {'Authorization': f'Token {token}'}

if not os.path.exists(file_path):
    os.makedirs(file_path)
for bot_id in bot_ids:
    participation_address = f'https://aiarena.net/api/match-participations/?bot={bot_id}'
    opponent_map = {}
    while participation_address:
        response = requests.get(participation_address, headers=auth)
        assert response.status_code == 200, 'Unexpected status_code returned from match-participations'
        participation = json.loads(response.text)
        participation_address = participation['next']
        for i in range(len(participation['results'])):
            match_id = participation['results'][i]['match']
            participant_number = participation['results'][i]['participant_number']
            existing_file = glob.glob(f"{file_path}{match_id}*")
            if participation["results"][i]['result'] == 'loss' and participation['results'][i]['elo_change']:
                response = requests.get(f'https://aiarena.net/api/results/?match={match_id}', headers=auth)
                assert response.status_code == 200, 'Unexpected status_code returned from results'
                match_details = json.loads(response.text)
                bot1_name = match_details['results'][0]['bot1_name']
                bot2_name = match_details['results'][0]['bot2_name']
                enemy_name = bot1_name
                if participant_number == 1:
                    enemy_name = bot2_name
                if enemy_name not in opponent_map:
                    opponent_map[enemy_name] = 0
                opponent_map[enemy_name] += participation['results'][i]['elo_change']
    print(f"{bot_id:<24}")
    print(f"{'Bot':<20} {'Elo':<4}")
    sorted_map = {k: v for k, v in sorted(opponent_map.items(), key=lambda item: item[1])[:5]}
    for key, value in sorted_map.items():
        print(f"{key:<20} {value:<4}")


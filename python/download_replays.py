import glob
import json
import os

import requests

# Download
bot_ids = [386]  # Ids on AI ARENA
token = os.environ['ARENA_API_TOKEN']  # Environment variable with token from: https://aiarena.net/profile/token/
file_path = './replays/'
auth = {'Authorization': f'Token {token}'}

if not os.path.exists(file_path):
    os.makedirs(file_path)
for bot_id in bot_ids:
    participation_address = f'https://aiarena.net/api/match-participations/?bot={bot_id}'
    while participation_address:
        response = requests.get(participation_address, headers=auth)
        assert response.status_code == 200, 'Unexpected status_code returned from match-participations'
        participation = json.loads(response.text)
        participation_address = participation['next']
        for i in range(len(participation['results'])):
            match_id = participation['results'][i]['match']
            existing_file = glob.glob(f"{file_path}{match_id}*")
            result = participation["results"][i]['result']
            if not existing_file and result == 'loss':
                response = requests.get(f'https://aiarena.net/api/results/?match={match_id}', headers=auth)
                assert response.status_code == 200, 'Unexpected status_code returned from results'
                match_details = json.loads(response.text)
                bot1_name = match_details['results'][0]['bot1_name']
                bot2_name = match_details['results'][0]['bot2_name']
                file_name = os.path.join(file_path, f"{match_id}_{bot1_name}_{bot2_name}.SC2Replay")
                replay_file = match_details['results'][0]['replay_file']
                if replay_file not in (None, 'null'):
                    print(f'Downloading match {file_name}')
                    replay = requests.get(replay_file, headers=auth)
                    with open(file_name, 'wb') as f:
                        f.write(replay.content)

import json
import os

import requests

# Download
bot_ids = [376]  # Ids on AI ARENA
token = os.environ['ARENA_API_TOKEN']  # Environment variable from: https://aiarena.net/profile/token/
file_path = './replays/'
auth = {'Authorization': f'Token {token}'}

if not os.path.exists(file_path):
    os.makedirs(file_path)
for bot_id in bot_ids:
    response = requests.get(f'https://aiarena.net/api/match-participations/?bot={bot_id}', headers=auth)
    assert response.status_code == 200, 'Unexpected status_code returned from match-participations'
    participation = json.loads(response.text)
    for i in range(len(participation['results'])):
        file_name = os.path.join(file_path, str(participation["results"][i]["match"]) + '.SC2Replay')
        if not os.path.isfile(file_name) and participation["results"][i]['result'] == 'loss':
            response = requests.get(f'https://aiarena.net/api/results/?match={participation["results"][i]["match"]}',
                                    headers=auth)
            assert response.status_code == 200, 'Unexpected status_code returned from results'
            match_details = json.loads(response.text)
            replay_file = match_details['results'][0]['replay_file']
            if replay_file not in (None, 'null'):
                print(f'Downloading match {participation["results"][i]["match"]}')
                replay = requests.get(replay_file, headers=auth)
                with open(file_name, 'wb') as f:
                    f.write(replay.content)

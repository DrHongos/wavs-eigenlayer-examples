# Sports Scores Oracle

This component fetches basketball game scores from the SportRadar API and returns them in a format suitable for blockchain oracles.

## Configuration

The SportRadar API key needs to be set in the Makefile:

```
SPORTRADAR_API_KEY?=your_api_key_here
```

Replace `your_api_key_here` with your actual SportRadar API key.

## Building

Build the component using the standard build process:

```
make build
```

## Usage

To fetch the scores for a specific basketball game, use:

```
make scores-exec GAME_ID="fa15684d-0966-46e7-a3f8-f1d378692109"
```

// this is mine, to test a soccer game finished
make scores-exec GAME_ID="sr:sport_event:57250777"

Where `fa15684d-0966-46e7-a3f8-f1d378692109` is the SportRadar game ID.

## Response Format

The oracle returns a JSON object with the following fields:

```json
{
  "id": "game-id",
  "home_team": "Home Team Name",
  "away_team": "Away Team Name",
  "home_score": 69,
  "away_score": 79,
  "status": "closed"
}
```

## API Documentation

This component uses the SportRadar NCAA Men's Basketball API. For more information, refer to the SportRadar API documentation.
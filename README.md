# Rust Chess Engine

**by: Nathan**

## Running the Program

double clicking the executable, this will start a game with you as white and "minimax AI" as black.

## Custom Matchups

Launch program from command line to choose who plays which side:

	chess_engine.exe <white> <black>

Available players:
- `human` - you play with the mouse
- `minimax` - default AI, searches and evaluates moves and picks 'the best' one
- `random` - plays a random legal move

Examples:

	chess_engine.exe human minimax
		-> this would be the default option
	chess_engine.exe minimax human
		-> this would be an AI for white and you as

# Rust Chess Engine

**by: Nathan E**

This program allows you to play untimed chess, either with multiple people (locally with another person) or against my AI (~2000 elo) I've created.



## Controls
- **Click** a piece to select it
- **Click** a destination to move
- **F** to flip the board

## Running the Program

Double-clicking the executable will start a game with you as white and "minimax AI" as black.

*read further for CLI usage/examples*

## Custom Matchups & Positions

### Launch program from command line with specification:

	chess_engine --white <white_player> --black <black_player> --depth <depth> --fen <fen_of_position>

**The program can be ran without direct specification of any of the fields above**

Ran without specification, white will always default to human player and black to minimax AI with an automatic depth of 5.

### Available players:
- `human` - you play with the mouse
- `minimax` - default AI, searches and evaluates moves and picks the 'best' one
- `random` - plays a random legal move

### Depth:

honestly, don't go higher than 5. I've experimented and pushed this default as high as I can go without causing much delay between moves however anything higher than 5 will leave you waiting minutes for a move, anything less than 3 will result in mostly inaccurate gameplay from the AI.

#### Examples:
- `./chess_engine` 	*This will start the default game, you as white and the AI as black. depth is set to 5*
- `./chess_engine --white minimax --black minimax ` 	*This will start a game with two AI's*
- `./chess_engine --white minimax --black minimax --depth 3` 	*This will start a game with two AI's with both depths being set to 3*
- `./chess_engine --white human --black minimax ` 	*This will start a game with a human playing for white and AI for black*
- `./chess_engine --fen "rnbqkbnr/pp1ppppp/8/2p5/4P3/8/PPPP1PPP/RNBQKBNR w KQkq c6 0 2" ` 	*This will start a game with a human playing white, AI playing black in a sicilian*


## About the AI

The AI's backbone is it's **minimax algorithm**, used to score moves. The minimax algorithm looks like this in pseudo-code:

```rust
fn minimax(board_state) -> i32{
	if depth == 0{
		return evaluate();
	}

	let eval = 0;
	if maximizing{
		let moves = get_all_moves();
		for mv in moves{
			make_move();
			new_eval = minimax(board_state); // this is the recursive part

			if new_eval > eval{ // did the move we just make improve our position
				eval = new_eval; // if so, this is the best eval
			}

			undo_move();
		}
	}else{
		let moves = get_all_moves();
		for mv in moves{
			make_move();
			new_eval = minimax(board_state); // this is the recursive part

			if new_eval < eval{ // did the move we just make improve our position
				eval = new_eval; // if so, this is the best eval
			}

			undo_move();
		}
	}
	eval
}

```

On top of this, I also use **Alpha-Beta Pruning**, which doesn't inherently lead to better moves per se however, it speeds up the search dramatically by reducing the amount of redundant, unfruitful nodes we have to calculate.

---

Regarding the evaluation function, It is pretty simple, I keep a table of values for which how good a position is for a piece. 2 tables, one for early-mid game and one for late game for each piece except knights because their positioning stays relatively the same within early-late game.. This is what the pawn table looks like:

```rust
const PAWN_TABLE: [[i32; 8]; 8] = [
    [ 0,    0,    0,    0,    0,    0,    0,    0   ],  // promotion 
    [ 90,   90,   90,   90,   90,   90,   90,   90  ],
    [ 25,   25,   50,   55,   55,   50,   25,   25  ],
    [ 10,   10,   25,   50,   50,   25,   10,   10  ],
    [ 5,    5,    25,   45,   45,   25,   5,    5   ],
    [ 5,    5,    10,   5,    5,   10,    5,    5   ],
    [ 5,    5,    5,   -10,  -10,   5,    5,    5   ],  // slight penalty for blocking center
    [ 0,    0,    0,    0,    0,    0,    0,    0   ],  // starting rank
];
```
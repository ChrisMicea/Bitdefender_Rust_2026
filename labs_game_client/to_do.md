1) fix temporary move logic (add ifs to avoid index boundaries exceeding)
2) restructure the movement logic like: use BFS (or any other algorithm) to calculate the path to take to the target position
    - remember (?) that path and move along it sequentially, turn-by-turn
3) implement enemy detection logic
4) introduce shooting function
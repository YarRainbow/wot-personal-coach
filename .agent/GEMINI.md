# Project Context: World of Tanks Personal Coach

This file serves as the source of truth for the project's context, architecture, and goals.

## 1. Project Overview
*   **Goal:** Create a legal World of Tanks modification that provides real-time, actionable advice to help players improve efficiency and productivity during matches.
*   **Core Value:** Runtime tips such as "Change ammo," "Hide from artillery," or "Fallback" based on the current situation.
*   **Target Audience:** Casual, low-skill players who are willing to subscribe for coaching help.

## 2. Technical Stack
### Core Technology
*   **Replay Parsing:** **Rust**.
    *   Chosen for speed and safety over C++. Avoids CMake complexity.
    *   References: `evido/wotreplay-parser`, `rajesh-rahul/wot-replay-tools`.
*   **Backend / Server:** **Elixir**.
    *   Chosen for high concurrency and scalability (B2C server).
*   **Machine Learning (ML):**
    *   **Research/Training:** **Python** (PyTorch/Scikit-learn) recommended for ecosystem.
    *   **Input:** General stats/meta weights + Replay data.
    *   **Goal:** Predict optimal positioning and strategies.
*   **Documentation:** Markdown with Mermaid diagrams.

### Architecture
1.  **Replay Parser (Rust):** High-performance tool to extract detailed data from replay files.
2.  **ML Pipeline (Python):** Processes parsed data to train models on positioning and game events.
3.  **Backend (Elixir):** Manages user subscriptions, serves model predictions (potentially), and handles business logic.
4.  **Client (Mod):** (Future) In-game overlay/notification system.

## 3. Product Features (MVP)
*   **Replay Parsing:** Robust parsing of replays to build the training dataset.
*   **Player Goals:** Users can configure specific coaching goals, such as:
    *   Max Damage
    *   Max Defense / Survival
    *   Max Assist Damage
    *   Max Win %
*   **Advice Engine:** The AI must tailor advice based on the selected goal.

## 4. Development Constraints & Guidelines
*   **Monorepo:** The project will be structured as a monorepo containing the Rust tools, Elixir backend, and Python ML scripts.
*   **Legal Compliance:** Strictly white-hat. No cheat mechanics (aimbots, seeing through walls). Only strategic advice based on available information.
*   **Data Strategy:** Start with manually downloaded replays. Scale to large replay datasets later.

# TheLoyalGame Chatbot

A full-stack discount chatbot built with Rust, Warp, and WebAssembly. The backend calculates loyalty-based discounts and stores data in Redis, while the frontend provides an interactive chat interface for users to input their phone number and bill amount, receive discounts, and submit feedback.

---

## Project Demo
Watch the demo video: [Click here to play TheLoyalGame Chatbot demo](https://raw.githubusercontent.com/YOUR_USERNAME/TheLoyalGame-Chatbot/main/TheLoyalGame%20ChatBot%20Video.mp4)

## Technical Blog: Building a Discount Chatbot with Rust, Warp, and WebAssembly

### Introduction

TheLoyalGame Chatbot is designed to help businesses implement loyalty-based discount strategies. Customers can input their phone number and bill amount, and the chatbot calculates a discount based on their purchase history. The application stores data in Redis, generates tokens for session management, and allows users to provide feedback with ratings and optional photos. The project demonstrates the power of Rust's performance, Warp's lightweight web framework, and WebAssembly's ability to bring Rust logic to the browser.

---

### Project Structure
```bash
The project is organized as follows:
├── Cargo.toml
├── src/
│   ├── main.rs          # Entry point for the Warp server (binary crate)
│   ├── lib.rs           # Library crate with core logic (get_response, token generation)
│   ├── handlers.rs      # Warp route handlers
│   └── (other files if any)
└── web/
├── index.html       # Frontend HTML
├── main.js          # Frontend JavaScript logic
├── botstyle.css     # CSS for styling the chatbot
└── wasm.js          # WebAssembly glue code (if used)
```

- **Backend**: The `src` directory contains the Rust code:
  - `main.rs`: Defines the Warp server and routes.
  - `lib.rs`: Contains the core business logic, such as calculating discounts and managing tokens.
  - `handlers.rs`: Implements the Warp route handlers for API endpoints.
- **Frontend**: The `web` directory contains the client-side code:
  - `index.html`: The main HTML page for the chatbot interface.
  - `main.js`: JavaScript logic for handling user input, making API requests, and updating the UI.
  - `botstyle.css`: Styles for the chatbot UI.
- **Dependencies**:
  - **Rust**: `warp`, `redis`, `serde`, `uuid`, `chrono`, `urlencoding`, `regex`, `base64`.
  - **Frontend**: No additional libraries; uses vanilla JavaScript and CSS.

---

### Implementation Details

#### Backend (Rust + Warp)

The backend is built using Rust and Warp, a lightweight and async web framework. It exposes three main endpoints:

1. **GET `/generate_token?phone=<phone>`**:
   - Generates a unique token for the user based on their phone number.
   - Stores the token in Redis with an expiry date (7 days).
   - Returns the token in JSON format: `{"token": "<uuid>"}`.

2. **GET `/get_discount/<business>/phone_number_amount/<phone,amount>/token/<token>`**:
   - Validates the token and calculates a discount based on the customer's purchase history.
   - Returns a plain text response: `Phone number: <phone>\n ; Final bill amount: <amount>\n ; Discount given: <percent>%`.

3. **POST `/submit_feedback`**:
   - Accepts multipart form data with `rating`, `note`, `phone`, and an optional `photo`.
   - Stores the feedback in Redis with a timestamp.
   - Returns a JSON response: `{"message": "Feedback received and stored!"}`.

**Key Logic in `lib.rs`**:
- `get_response`: Calculates the discount by checking the customer's purchase history from the previous week (stored in Redis). It applies a 3% pooling mechanism to distribute discounts among eligible customers.
- `generate_and_store_token`: Creates a UUID token, sets an expiry date, and stores it in Redis.
- `persist_data_to_redis` and `fetch_data_from_redis`: Utility functions for interacting with Redis.

**Challenges**:
- **Module Resolution Issue**: Initially, the project faced an `E0432: unresolved imports` error because the `lib.rs` module wasn't correctly recognized by the binary crate (`main.rs`). This was resolved by explicitly defining the `[lib]` and `[[bin]]` sections in `Cargo.toml` and fixing the import paths.
- **Response Parsing**: The server returned a plain text response wrapped in `warp::reply::json`, causing a `Content-Type` mismatch. This was fixed by sending the response as plain text with the correct `Content-Type: text/plain`.

#### Frontend (HTML + JavaScript + CSS)

The frontend is a simple chatbot interface where users can input their phone number and bill amount. It communicates with the backend via HTTP requests and updates the UI dynamically.

**Key Features**:
- **Input Handling**: Users enter `phone,amount` (e.g., `9898989898, 600.50`). The `parseInput` function splits and validates the input.
- **Token Management**: The `generateToken` function fetches a token from the backend and stores it in `localStorage` for session persistence.
- **Discount Fetching**: The `fetchDiscount` function makes a request to the `/get_discount` endpoint and displays the result using `formatDiscountResponse`.
- **Feedback Form**: After receiving a discount, users are prompted to rate their experience. The `showRatingForm` function displays a form with star ratings, a text note, and an optional photo upload.

**Challenges**:
- **Response Parsing**: The frontend initially failed to parse the server's plain text response correctly, displaying `N/A` for phone number and bill amount. This was fixed by improving the `formatDiscountResponse` function to split the response on `\n ;` and handle the format robustly.
- **Undefined Messages**: The chat displayed `undefined` messages after certain actions (e.g., showing the rating prompt or submitting feedback). This was resolved by adding error handling in `addMessage` and `addMessageElement`, and wrapping event handlers in `try-catch` blocks.

#### Redis Integration

Redis is used to store:
- Tokens (`token:<uuid>`, `phone:<phone>:token`, `<business>_token_<uuid>`).
- Weekly purchase data (`<business>___<date>`).
- Feedback (`feedback:<phone>:<timestamp>`).

**Challenge**:
- Ensuring token expiry and validation was tricky. The `get_response` function checks the token's expiry date and validates it against the stored value in Redis. If the token is expired or invalid, it returns an error message.

---

### How to Run the Application

Follow these steps to set up and run TheLoyalGame Chatbot on your local machine.

#### Prerequisites
- **Rust**: Install Rust by following the instructions at [rust-lang.org](https://www.rust-lang.org/tools/install).
- **Redis**: Install Redis by following the instructions at [redis.io](https://redis.io/docs/getting-started/installation/). On Windows, you can use WSL or download a Windows build.
- **Node.js** (for `live-server`): Install Node.js from [nodejs.org](https://nodejs.org/). Then install `live-server` globally:
  ```bash
  npm install -g live-server

### Step 1: Clone the Repository
Assuming you have the project files in D:\Software Engineering Intern - 2025\V1\theloyalgame, navigate to the directory:

```bash
cd D:\Software Engineering Intern - 2025\V1\theloyalgame
```
### Step 2: Start Redis
Start the Redis server:

```bash
redis-server
```
Verify Redis is running:

```bash
redis-cli
127.0.0.1:6379> ping
PONG
```
### Step 3: Build and Run the Backend
Build and run the Rust backend:

```bash
cargo run
```
The server will start on http://0.0.0.0:3030. You should see:

```bash
Server starting on http://0.0.0.0:3030
```

### Step 4: Run the Frontend
Navigate to the web directory and start a local server using live-server:

```bash
cd web
live-server --port=8000
```

The frontend will be available at http://127.0.0.1:8000.

### Step 5: Test the Application
Open http://127.0.0.1:8000 in your browser.
Enter a phone number and bill amount (e.g., 9898989898, 600.50) in the chat input and press Enter.
The chatbot will:
Generate a token (if needed).
Fetch and display the discount details.
Prompt for feedback.
Submit feedback by selecting a rating, adding a note, and optionally uploading a photo.
Expected Output:

```bash
Hey! I'm TheLoyalGame chatbot!
Enter phone_number, bill_amount to get a discount suggestion.
Example: 9898989898, 600.50

Note: Free version offers weekly discounts.
Go premium for custom loyalty strategies.
Email business@theloyalgame.com

9898989898, 600.50
🔑 Generating a new token...
⏳ Fetching your discount...
🎉 Discount Details:
- Phone Number: 9898989898
- Final Bill Amount: $600.50
- Discount Given: 0.00%
🎉 Task Completed!
Rate your experience?

Yes 😊
No 😞
[Rating Form]
✅ Thank you for your feedback! 🎉

```
### Step 6: Debugging
Backend Logs: Check the terminal where cargo run is running for server logs.
Frontend Logs: Open the browser's DevTools (F12 → Console) to see JavaScript logs.
Redis Data: Use redis-cli to inspect stored data:

```bash
redis-cli
127.0.0.1:6379> keys *
```

### Challenges and Solutions
1. Module Resolution in Rust:
- Issue: The E0432: unresolved imports error occurred because lib.rs wasn't recognized by main.rs.
- Solution: Added [lib] and [[bin]] sections to Cargo.toml and fixed imports to use the crate name (chatbot_rust_wasm).
2. Response Parsing in Frontend:
- Issue: The frontend displayed N/A for phone number and bill amount because the formatDiscountResponse function couldn't parse the server's plain text response.
- Solution: Updated formatDiscountResponse to split the response on \n ; and handle the format correctly.
3. Undefined Messages in Chat:
- Issue: The chat displayed undefined messages after certain actions due to uncaught errors in event handlers.
- Solution: Added error handling in addMessage, addMessageElement, and event handlers (showRatingPrompt, handleFeedbackSubmit).
4. Content-Type Mismatch:
- Issue: The server used warp::reply::json for a plain text response, causing a Content-Type mismatch.
- Solution: Changed the server to send the response as plain text with Content-Type: text/plain.

### Future Improvements
- Return JSON from the Server: Modify get_response to return a JSON object instead of plain text, making parsing easier on the frontend.
- Add Authentication: Implement proper user authentication instead of hardcoding the business name (test102).
- Improve Discount Logic: Enhance the discount calculation to consider more factors, such as customer loyalty tiers or purchase frequency.
- Add Tests: Write more unit and integration tests for the backend and frontend.

### Conclusion
TheLoyalGame Chatbot is a practical example of building a full-stack application with Rust, Warp, and WebAssembly. It demonstrates how to handle HTTP requests, manage sessions with Redis, and create an interactive frontend with JavaScript. Despite initial challenges like module resolution and response parsing, the project now works seamlessly, providing a smooth user experience for calculating discounts and collecting feedback.

```bash

This `README.md` file includes the full technical blog and detailed instructions for running the project. You can place it in the root directory of your project (`D:\Software Engineering Intern - 2025\V1\theloyalgame\README.md`) to serve as both documentation and a blog. Let me know if you'd like to make any adjustments!
```
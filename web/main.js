// main.js

// Base URL for API requests
const baseURL = "http://127.0.0.1:3030";

// State object to track application state
const state = {
  username: null,
  token: localStorage.getItem("authToken") || "no-token",
  feedbackPromptCount: 0,
  feedbackDeclinedCount: 0,
  step: "phone", // New step tracker: "phone" -> "amount" -> "done"
  phone: null,   // Store phone number temporarily
};

// Utility functions
function createElement(tag, className, innerHTML) {
  const element = document.createElement(tag);
  if (className) element.className = className;
  if (innerHTML) element.innerHTML = innerHTML;
  return element;
}

function addMessage(message, isBot) {
  const chatList = document.getElementById("chat-list");
  if (!chatList) {
    console.error("Chat list element not found!");
    return;
  }
  const messageElement = createElement(
    "div",
    isBot ? "talk-bubble-bot tri-right round right-in" : "talk-bubble-user tri-right round left-in"
  );
  const textElement = createElement("div", "bot-text");
  textElement.innerHTML = `<p>${message}</p>`;
  messageElement.appendChild(textElement);
  chatList.appendChild(messageElement);
  messageElement.scrollIntoView({ behavior: "smooth" });
}

function addMessageElement(element, isBot) {
  const chatList = document.getElementById("chat-list");
  if (!chatList) {
    console.error("Chat list element not found!");
    return;
  }
  const messageElement = createElement(
    "div",
    isBot ? "talk-bubble-bot tri-right round right-in" : "talk-bubble-user tri-right round left-in"
  );
  messageElement.appendChild(element);
  chatList.appendChild(messageElement);
  messageElement.scrollIntoView({ behavior: "smooth" });
}

function validatePhone(phone) {
  return phone && phone.length === 10 && /^\d+$/.test(phone);
}

function validateAmount(amount) {
  return amount && !isNaN(amount) && parseFloat(amount) > 0;
}

// Fetch discount from the server
async function fetchDiscount(phone, amount) {
  console.log("Current token state:", state.token);
  const url = `${baseURL}/get_discount/${encodeURIComponent(
    state.username
  )}/phone_number_amount/${encodeURIComponent(phone)},${encodeURIComponent(
    amount
  )}/token/${encodeURIComponent(state.token)}`;
  console.log("Fetching discount with URL:", url);
  return fetch(url, { headers: { Accept: "text/plain" } });
}

// Generate a new token
async function generateToken(phone) {
  try {
    const response = await fetch(
      `${baseURL}/generate_token?phone=${encodeURIComponent(phone)}`,
      {
        method: "GET",
        headers: { Accept: "application/json" },
      }
    );
    if (!response.ok) {
      throw new Error(`Failed to generate token: ${response.statusText}`);
    }
    const data = await response.json();
    if (!data.token) {
      throw new Error("Token not found in response");
    }
    console.log("Generated token details:", data);
    return data.token;
  } catch (error) {
    console.error("Token generation error:", error);
    throw error;
  }
}

// Format the discount response
function formatDiscountResponse(data) {
  if (!data || typeof data !== "string") {
    console.error("Invalid response data:", data);
    return "⚠️ Error: Invalid response from server.";
  }

  console.log("Raw Response Data:", data);

  if (data.includes("Not authorized / Token expired")) {
    return "⚠️ Session expired. Please try again!";
  }

  const rawLines = data.split("\n ; ");
  console.log("Raw Lines:", rawLines);

  const result = {};

  rawLines.forEach((line) => {
    const cleanedLine = line.trim().replace(/;/g, "");
    console.log("Processing Line:", cleanedLine);

    const [key, value] = cleanedLine.split(/:\s*/, 2);
    if (key && value) {
      const normalizedKey = key.toLowerCase().replace(/\s+/g, "_");
      result[normalizedKey] = value.trim();
      console.log(`Parsed Key: ${normalizedKey}, Value: ${value.trim()}`);
    } else {
      console.log("Failed to parse line:", cleanedLine);
    }
  });

  console.log("Parsed Result:", result);

  const phoneNumber = result.phone_number || "N/A";
  const finalBillAmount = result.final_bill_amount
    ? parseFloat(result.final_bill_amount).toFixed(2)
    : "N/A";
  const discountGiven = result.discount_given
    ? result.discount_given.replace("%", "").trim()
    : "0.00";
  const hasTransaction = result.has_transaction || "false";

  let message = `
    🎉 Discount Details:<br>
    - Phone Number: ${phoneNumber}<br>
    - Final Bill Amount: $${finalBillAmount}<br>
    - Discount Given: ${discountGiven}%
  `;

  if (parseFloat(discountGiven) === 0) {
    if (hasTransaction === "true") {
      message += `<br>ℹ️ Note: You've already received a discount this week. Try again next week!`;
    } else {
      message += `<br>ℹ️ Note: The weekly discount pool has been exhausted. Try again next week!`;
    }
  }

  return message;
}

// Show the rating prompt
function showRatingPrompt(phone) {
  console.log("Showing rating prompt for phone:", phone);
  const div = createElement(
    "div",
    "rating-prompt",
    `
      <p aria-live="polite">🎉 Task Completed!<br>Rate your experience?</p>
      <div class="button-group">
        <button class="btn yes" aria-label="Rate Yes">Yes 😊</button>
        <button class="btn no" aria-label="Rate No">No 😞</button>
      </div>
    `
  );
  addMessageElement(div, true);
  const yesButton = div.querySelector(".yes");
  const noButton = div.querySelector(".no");
  if (yesButton && noButton) {
    yesButton.addEventListener("click", () => {
      try {
        console.log("Yes button clicked, showing rating form");
        state.feedbackDeclinedCount = 0;
        div.style.display = "none";
        const existingForms = document.querySelectorAll(".rating-form");
        existingForms.forEach((form) => (form.style.display = "none"));
        showRatingForm(phone, true);
      } catch (error) {
        console.error("Error in Yes button handler:", error);
        addMessage("⚠️ Failed to show rating form. Please try again!", true);
      }
    });
    noButton.addEventListener("click", () => {
      try {
        console.log("No button clicked, hiding prompt");
        state.feedbackDeclinedCount++;
        div.style.display = "none";
        addMessage("Thank you for using our service! 😊", true);
      } catch (error) {
        console.error("Error in No button handler:", error);
        addMessage("⚠️ Failed to show rating form. Please try again!", true);
      }
    });
  } else {
    console.error("Failed to find Yes/No buttons in rating prompt");
    addMessage(
      "⚠️ Failed to initialize rating prompt. Please try again!",
      true
    );
  }
}

// Show the rating form
function showRatingForm(phone, show) {
  if (!show) {
    addMessage("Thank you for using our service! 😊", true);
    return;
  }

  const form = createElement(
    "div",
    "rating-form",
    `
      <h3 aria-label="Rate Your Experience">🌟 Rate Your Experience 🌟</h3>
      <div class="star-rating" role="radiogroup" aria-label="Rating Stars">
        ${Array.from(
          { length: 5 },
          (_, i) =>
            `<span class="star" data-value="${
              i + 1
            }" role="radio" aria-checked="false">★</span>`
        ).join("")}
      </div>
      <input type="hidden" id="rating-value" value="0">
      <textarea id="rating-note" placeholder="Your feedback..." aria-label="Feedback Note" spellcheck="true"></textarea>
      <div class="file-upload">
        <input type="file" id="rating-photo" accept="image/*" aria-label="Upload Photo">
        <label for="rating-photo">📸 Upload Photo (Optional)</label>
        <div class="preview" aria-live="polite"></div>
      </div>
      <button class="submit-btn" aria-label="Submit Feedback">Submit Feedback 🚀</button>
    `
  );

  addMessageElement(form, true);
  initStarRating(form);
  initFileUpload(form);
  form
    .querySelector(".submit-btn")
    .addEventListener("click", () => handleFeedbackSubmit(phone, form));
}

// Initialize star rating with blue color on hover and selection
function initStarRating(form) {
  const stars = form.querySelectorAll(".star");
  const ratingInput = form.querySelector("#rating-value");

  stars.forEach((star) => {
    star.addEventListener("click", () => {
      const value = star.getAttribute("data-value");
      ratingInput.value = value;

      stars.forEach((s, index) => {
        const isSelected = (index + 1) <= value;
        s.setAttribute("aria-checked", isSelected);
        s.classList.toggle("selected", isSelected);
      });
    });

    star.addEventListener("mouseover", () => {
      stars.forEach((s, index) => {
        if (index + 1 <= star.getAttribute("data-value")) {
          s.classList.add("hover");
        } else {
          s.classList.remove("hover");
        }
      });
    });

    star.addEventListener("mouseout", () => {
      stars.forEach((s) => s.classList.remove("hover"));
    });
  });
}

// Initialize file upload with validation
function initFileUpload(form) {
  const fileInput = form.querySelector("#rating-photo");
  const preview = form.querySelector(".preview");

  fileInput.addEventListener("change", () => {
    const file = fileInput.files[0];
    if (file) {
      if (!file.type.startsWith("image/")) {
        addMessage("⚠️ Please upload an image file!", true);
        fileInput.value = "";
        preview.innerHTML = "";
        return;
      }
      const maxSize = 1 * 1024 * 1024; // 1 MB
      if (file.size > maxSize) {
        addMessage(`⚠️ Image size (${(file.size / (1024 * 1024)).toFixed(2)} MB) exceeds 1 MB limit!`, true);
        fileInput.value = "";
        preview.innerHTML = "";
        return;
      }

      const reader = new FileReader();
      reader.onload = (e) => {
        preview.innerHTML = `<img src="${e.target.result}" alt="Uploaded photo preview" style="max-width: 100px; max-height: 100px;" />`;
      };
      reader.readAsDataURL(file);
    } else {
      preview.innerHTML = "";
    }
  });
}

// Handle feedback submission
async function handleFeedbackSubmit(phone, form) {
  const rating = form.querySelector("#rating-value").value;
  const comment = form.querySelector("#rating-note").value;
  const photoFile = form.querySelector("#rating-photo").files[0];

  if (parseInt(rating) === 0) {
    addMessage("⚠️ Please select a rating!", true);
    return;
  }

  console.log("Submitting feedback with data:", {
    phone_number: phone,
    rating: rating,
    comment: comment,
    photo: photoFile ? photoFile.name : "No photo",
  });

  try {
    let response;
    if (photoFile) {
      const formData = new FormData();
      formData.append("phone_number", phone);
      formData.append("rating", parseInt(rating));
      formData.append("comment", comment);
      formData.append("photo", photoFile);

      response = await fetch(`${baseURL}/submit_feedback`, {
        method: "POST",
        body: formData,
        signal: AbortSignal.timeout(10000),
      });
    } else {
      const jsonData = {
        phone_number: phone,
        rating: parseInt(rating),
        comment: comment,
      };

      response = await fetch(`${baseURL}/submit_feedback`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(jsonData),
        signal: AbortSignal.timeout(10000),
      });
    }

    if (!response.ok) {
      const errorText = await response.text();
      console.error(`Feedback submission failed with status ${response.status}: ${errorText}`);
      if (response.status === 413) {
        throw new Error("File size too large. Please upload a smaller image (max 1 MB).");
      }
      throw new Error(`Failed to submit feedback: ${response.status} - ${errorText}`);
    }

    console.log("Feedback submitted successfully, closing form");
    form.style.display = "none";
    addMessage("✅ Thank you for your feedback! 🎉", true);
  } catch (error) {
    console.error("Feedback submission error:", error);
    form.style.display = "none";
    addMessage(`⚠️ ${error.message || "Failed to submit feedback. Please try again!"}`, true);
  }
}

// Handle form submission with step-by-step input
async function handleSubmit(event) {
  event.preventDefault();
  const input = document.getElementById("input-message");
  if (!input) {
    console.error("Input message element not found!");
    return;
  }
  const inputText = input.value.trim();
  if (!inputText) return;

  input.value = "";
  input.focus();
  addMessage(inputText, false);

  if (state.step === "phone") {
    if (!validatePhone(inputText)) {
      addMessage(
        "⚠️ Please enter a valid 10-digit phone number (e.g., 9898989898)",
        true
      );
      return;
    }
    state.phone = inputText;
    state.step = "amount";
    addMessage("📏 Please enter the bill amount (e.g., 600.50)", true);
  } else if (state.step === "amount") {
    if (!validateAmount(inputText)) {
      addMessage(
        "⚠️ Please enter a valid amount (e.g., 600.50)",
        true
      );
      return;
    }
    const amount = parseFloat(inputText);
    state.step = "done";

    try {
      state.username = "test102";
      if (!state.token || state.token === "no-token") {
        addMessage("🔑 Generating a new token...", true);
        state.token = await generateToken(state.phone);
        localStorage.setItem("authToken", state.token);
        console.log("Generated and Stored Token:", state.token);
        addMessage("✅ New token generated successfully!", true);
      }

      addMessage("⏳ Fetching your discount...", true);
      const response = await fetchDiscount(state.phone, amount);
      if (!response.ok)
        throw new Error(`HTTP error! status: ${response.status}`);

      const data = await response.text();
      console.log("Response Data:", data);
      if (data.includes("Not authorized / Token expired")) {
        addMessage(
          "⚠️ Your session has expired. Generating a new token...",
          true
        );
        state.token = null;
        localStorage.removeItem("authToken");
        state.token = await generateToken(state.phone);
        localStorage.setItem("authToken", state.token);
        console.log("Regenerated Token:", state.token);
        addMessage("✅ New token generated successfully!", true);
        const retryResponse = await fetchDiscount(state.phone, amount);
        if (!retryResponse.ok)
          throw new Error(`Retry failed! status: ${retryResponse.status}`);
        const retryData = await retryResponse.text();
        const formattedResponse = formatDiscountResponse(retryData);
        addMessage(formattedResponse, true);
        addMessage("✅ Discount applied successfully!", true);
      } else {
        const formattedResponse = formatDiscountResponse(data);
        addMessage(formattedResponse, true);
        addMessage("✅ Discount applied successfully!", true);
      }

      if (state.feedbackDeclinedCount < 3) {
        try {
          showRatingPrompt(state.phone);
          state.feedbackPromptCount++;
        } catch (error) {
          console.error("Error in showRatingPrompt:", error);
          addMessage(
            "⚠️ Failed to show rating prompt. Please try again!",
            true
          );
        }
      } else {
        addMessage("Thank you for using our service! 😊", true);
      }

      // Reset state for the next interaction
      state.step = "phone";
      state.phone = null;
    } catch (error) {
      console.error("Error in handleSubmit:", error);
      addMessage("⚠️ Failed to get discount. Please try again!", true);
      state.token = null;
      localStorage.removeItem("authToken");
      state.step = "phone"; // Reset step on error
      state.phone = null;
    }
  }
}

// Initialize the application after the DOM is fully loaded
document.addEventListener("DOMContentLoaded", () => {
  const form = document.getElementById("chat-form");
  if (!form) {
    console.error("Chat form element not found!");
    return;
  }

  // Initial message
  addMessage("Hey! I'm TheLoyalGame chatbot!<br>📞 Please enter your phone number (e.g., 9898989898)", true);

  // Event listener for form submission
  form.addEventListener("submit", handleSubmit);
});
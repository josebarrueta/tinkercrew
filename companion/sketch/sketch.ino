#include <Arduino_Modulino.h>
#include <Arduino_RouterBridge.h>
#include <Arduino_LED_Matrix.h>

ModulinoButtons buttons;
ModulinoPixels pixels;
ModulinoBuzzer buzzer;
ModulinoDistance distanceSensor;
ArduinoLEDMatrix matrix;

bool buttonsConnected = false;
bool pixelsConnected = false;
bool buzzerConnected = false;
bool distanceConnected = false;
unsigned long pixelsUntil = 0;
unsigned long matrixUntil = 0;
unsigned long nextInputReport = 0;
unsigned long controlLeaseUntil = 0;

void clearMatrix() {
  uint8_t empty[8][13] = {};
  matrix.renderBitmap(empty, 8, 13);
}

bool i2cPresent(uint8_t address) {
  Wire1.beginTransmission(address);
  return Wire1.endTransmission() == 0;
}

bool tc_control_lease(int leaseMs) {
  if (leaseMs < 100 || leaseMs > 2000) return false;
  controlLeaseUntil = millis() + leaseMs;
  return true;
}

bool tc_safe_state() {
  if (buzzerConnected) buzzer.noTone();
  if (pixelsConnected) {
    pixels.clear();
    pixels.show();
  }
  clearMatrix();
  pixelsUntil = 0;
  matrixUntil = 0;
  controlLeaseUntil = 0;
  return true;
}

bool tc_pixels_set(int red, int green, int blue, int brightness, int durationMs) {
  if (!pixelsConnected || red < 0 || red > 255 || green < 0 || green > 255 ||
      blue < 0 || blue > 255 || brightness < 0 || brightness > 50 ||
      durationMs < 1 || durationMs > 10000) return false;
  for (int index = 0; index < 8; index++) {
    pixels.set(index, red, green, blue, brightness);
  }
  pixels.show();
  pixelsUntil = millis() + durationMs;
  return true;
}

bool tc_buzzer_play(int frequencyHz, int durationMs, int volume) {
  if (!buzzerConnected || frequencyHz < 100 || frequencyHz > 4000 ||
      durationMs < 1 || durationMs > 3000 || volume < 0 || volume > 50) return false;
  // Modulino Buzzer has no volume control. The bounded value is accepted for protocol consistency.
  buzzer.tone(frequencyHz, durationMs);
  return true;
}

bool tc_matrix_draw(String flattened, int durationMs) {
  if (flattened.length() != 104 || durationMs < 1 || durationMs > 10000) return false;
  uint8_t frame[8][13] = {};
  for (int row = 0; row < 8; row++) {
    for (int column = 0; column < 13; column++) {
      char value = flattened.charAt(row * 13 + column);
      if (value != '0' && value != '1') return false;
      frame[row][column] = value == '1';
    }
  }
  matrix.renderBitmap(frame, 8, 13);
  matrixUntil = millis() + durationMs;
  return true;
}

bool tc_self_test() {
  if (pixelsConnected) {
    for (int index = 0; index < 8; index++) pixels.set(index, 0, 64, 64, 20);
    pixels.show();
    pixelsUntil = millis() + 1000;
  }
  if (buzzerConnected) buzzer.tone(440, 250);
  uint8_t check[8][13] = {};
  for (int index = 0; index < 8; index++) check[index][index] = 1;
  matrix.renderBitmap(check, 8, 13);
  matrixUntil = millis() + 1000;
  return true;
}

void setup() {
  Bridge.begin();
  Modulino.begin();
  matrix.begin();

  buttonsConnected = buttons.begin();
  pixelsConnected = pixels.begin();
  buzzerConnected = buzzer.begin();
  distanceConnected = distanceSensor.begin();

  Bridge.provide_safe("tc_control_lease", tc_control_lease);
  Bridge.provide_safe("tc_safe_state", tc_safe_state);
  Bridge.provide_safe("tc_pixels_set", tc_pixels_set);
  Bridge.provide_safe("tc_buzzer_play", tc_buzzer_play);
  Bridge.provide_safe("tc_matrix_draw", tc_matrix_draw);
  Bridge.provide_safe("tc_self_test", tc_self_test);
  tc_safe_state();
}

void loop() {
  unsigned long now = millis();
  if (controlLeaseUntil && (long)(now - controlLeaseUntil) >= 0) {
    tc_safe_state();
  }
  if (pixelsUntil && (long)(now - pixelsUntil) >= 0) {
    pixels.clear();
    pixels.show();
    pixelsUntil = 0;
  }
  if (matrixUntil && (long)(now - matrixUntil) >= 0) {
    clearMatrix();
    matrixUntil = 0;
  }

  if ((long)(now - nextInputReport) >= 0) {
    nextInputReport = now + 250;
    bool buttonsPresent = i2cPresent(0x3E);
    bool pixelsPresent = i2cPresent(0x36);
    bool buzzerPresent = i2cPresent(0x1E);
    bool distancePresent = i2cPresent(0x29);
    if (buttonsPresent && !buttonsConnected) buttonsConnected = buttons.begin();
    if (pixelsPresent && !pixelsConnected) pixelsConnected = pixels.begin();
    if (buzzerPresent && !buzzerConnected) buzzerConnected = buzzer.begin();
    if (distancePresent && !distanceConnected) distanceConnected = distanceSensor.begin();
    buttonsConnected = buttonsConnected && buttonsPresent;
    pixelsConnected = pixelsConnected && pixelsPresent;
    buzzerConnected = buzzerConnected && buzzerPresent;
    distanceConnected = distanceConnected && distancePresent;

    if (buttonsConnected) {
      buttons.update();
      Bridge.notify("tc_buttons", bool(buttons.isPressed(0)), bool(buttons.isPressed(1)), bool(buttons.isPressed(2)));
    } else {
      Bridge.notify("tc_disconnected", "buttons");
    }
    if (distanceConnected && distanceSensor.available()) {
      Bridge.notify("tc_distance", int(distanceSensor.get()));
    } else if (!distanceConnected) {
      Bridge.notify("tc_disconnected", "distance");
    }
    Bridge.notify("tc_capability_health", "pixels", pixelsConnected ? "operational" : "disconnected");
    Bridge.notify("tc_capability_health", "buzzer", buzzerConnected ? "operational" : "disconnected");
    Bridge.notify("tc_capability_health", "matrix", "operational");
  }
}

CREATE TABLE IF NOT EXISTS guess (
  user_id INT NOT NULL,
  guess TEXT NOT NULL,
  day TEXT NOT NULL,
  FOREIGN KEY (user_id) REFERENCES user(id)

  PRIMARY KEY (user_id, guess, day)
);

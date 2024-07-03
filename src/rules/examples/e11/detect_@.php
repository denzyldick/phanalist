<?php

class E11 {
  public function example(): bool {
    if (true) {
      if (true) {
        @$this->fetch();
      }
    }
    return false;
  }

  public function fetch(): int {
    return 1;
  }
}

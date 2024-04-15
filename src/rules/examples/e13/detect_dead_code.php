<?php

namespace DeadCode {

  class Test {

    public function __construct() {
    }

    private function isNotCalled(): bool {

      $this->test2();
      return true;
    }

    private static function test() {
      $this->isNotCalled();
    }

    public function test2() {
    }
  }
}

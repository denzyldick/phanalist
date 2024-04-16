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
    }

    private function test2() {

      static::test();
      $this->test2();
    }

    public function ignore() {
    }
  }
}

<?php

/**
 *
 */

namespace DeadCode {

  class Test
  {

    private function isNotCalled(): bool
    {

      $this->test2();
      $this->test2();
      return true;
    }
  }
}

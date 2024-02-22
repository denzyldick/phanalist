<?php

class E11
{
  public function example(): bool
  {
    if ($this->fetch()) {
      $a = true;
    }
    return $a;
  }


  public function fetch(): int
  {
    return 1;
  }
}

<?php

namespace App\Service\e12;

class DefineInConstructor {

    public function __construct(private bool $debug = false) {
    }
}
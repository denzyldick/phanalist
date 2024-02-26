<?php

namespace App\Service\e12;

class SetInStaticMethod {

    private static int $counter = 0;

    public static function setCounter(int $counter): void {
        self::$counter = $counter;
    }
}
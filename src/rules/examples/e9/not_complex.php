<?php

namespace Test\e9;

class NotComplex {
    public function greeting(?string $language): string {
        if (null === $language) {
            $language = $this->getDefaultLanguage();
        }

        if ('ua' === $language) {
            return 'Привіт!';
        } else if ('es' === $language) {
            return 'Hola';
        } else {
            return 'Hello';
        }
    }

    private function getDefaultLanguage(): string {
        return 'en';
    }
}
# TinkerCrew

TinkerCrew is a local-first cooperative maker game in which one family crew builds physical creations and gives them behavior through a Mac application and a connected board.

## Language

**Crew**:
The family group that shares missions, roles, and progress.
_Avoid_: Account, classroom, team

**Crew member**:
A person participating in the Crew, with a current role and challenge track.
_Avoid_: User, child account, player account

**Mission**:
A guided adventure that combines a story, a physical build, observable behavior, remixing, and reflection.
_Avoid_: Lesson, tutorial, project

**Board**:
The specific Arduino UNO Q selected by the Crew for setup, recovery, or play.
_Avoid_: Generic device, controller

**Companion**:
The single reusable service on the Board that exposes bounded hardware capabilities to TinkerCrew.
_Avoid_: Mission app, board server, agent

**Pairing**:
The revocable trust relationship between one TinkerCrew installation and one Board, established with physical USB access.
_Avoid_: Login, sign-in, Bluetooth pairing

**Control session**:
A short-lived authenticated connection through which TinkerCrew observes and controls a paired Board.
_Avoid_: Login session, socket

**Capability**:
A hardware behavior the Companion reports as available, such as distance sensing, buttons, pixels, buzzer, or matrix drawing.
_Avoid_: Peripheral, feature flag

**Observable**:
A timestamped value or event reported by a Capability, including its freshness and health.
_Avoid_: Sensor data, telemetry field

**Command**:
A bounded request to change Board output state through a Capability.
_Avoid_: Shell command, instruction

**Safe state**:
The documented output condition the Companion enters when control is lost or a safety limit is reached.
_Avoid_: Reset state, default state
